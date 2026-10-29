//! Classes ([class], [class.mem], [class.derived], [class.base.init];
//! spec/parse.md §2.4, §4.1): class-specifier bodies, base clauses,
//! member-declarations, ctor-initializers, and the delayed parsing of
//! complete-class contexts ([class.mem.general]/7).
//!
//! Inside the outermost class being defined, member function bodies (with
//! their ctor-initializers), default arguments, *noexcept-specifier*s and
//! default member initializers are saved as tokens and re-parsed after the
//! outermost `}` (clang's `LateParsedDeclaration`s), in clang's three
//! phases: method declarations, member initializers, method bodies.

use std::mem;

use kiln_ast::decl::{Access, DeclFlags, DeclKind, FriendTarget, InitStyle};
use kiln_ast::names::OverloadedOperator;
use kiln_ast::types::TagKeyword;
use kiln_ast::{DeclId, QualType};
use kiln_diag::DiagId;
use kiln_lex::{Punct, TokKind, Token};
use kiln_sema::{
    BaseSpecInfo, Chunk, DeclSpec, Declarator, DeclaratorContext, Initializer, MemInitId,
    MemInitInfo, NameClass, SpecialName, StorageSpec, TypeSpecKind,
};
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::decl::is_untyped_special;
use crate::keywords::Kw;
use crate::parser::{Parser, quote};

/// Saved tokens and the location of the `Eof` sentinel that ends their
/// replay.
pub(crate) struct LateToks {
    toks: Vec<Token>,
    end: Loc,
}

/// The saved parts of a member function declarator: its default
/// arguments (by parameter index) and its *noexcept-specifier*. The
/// `delay_*` switches tell the declarator parser whether to save them.
#[derive(Default)]
pub(crate) struct LateFnParts {
    pub(crate) default_args: Vec<(usize, LateToks)>,
    pub(crate) noexcept: Option<LateToks>,
    pub(crate) delay_args: bool,
    pub(crate) delay_noexcept: bool,
}

impl LateFnParts {
    fn is_empty(&self) -> bool {
        self.default_args.is_empty() && self.noexcept.is_none()
    }
}

/// A class being defined and its saved complete-class contexts (clang's
/// `ParsingClass`).
pub(crate) struct LateClass {
    record: DeclId,
    items: Vec<LateItem>,
}

/// One saved complete-class context, in declaration order.
enum LateItem {
    /// Default arguments and the *noexcept-specifier* of `f`.
    Method { f: DeclId, parts: LateFnParts },
    /// The default member initializer of `field` (from its `=` or `{`).
    Nsdmi { field: DeclId, toks: LateToks },
    /// The body of `f`, from its `:`, `try` or `{`.
    Body { f: DeclId, ctor: bool, toks: LateToks },
    /// A nested class with saved contexts of its own.
    Nested(LateClass),
}

/// The replay phases, in clang's order.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Phase {
    Method,
    Nsdmi,
    Body,
}

impl LateClass {
    /// True if this class or a nested one has items of phase `ph`.
    fn has(&self, ph: Phase) -> bool {
        self.items.iter().any(|i| match i {
            LateItem::Method { .. } => ph == Phase::Method,
            LateItem::Nsdmi { .. } => ph == Phase::Nsdmi,
            LateItem::Body { .. } => ph == Phase::Body,
            LateItem::Nested(c) => c.has(ph),
        })
    }
}

/// What a saved initializer ends at (clang's `CachedInitKind`).
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum InitKind {
    /// A default argument: ends at `,` or `)`.
    DefaultArg,
    /// A default member initializer: ends at `,` or `;`.
    Nsdmi,
}

/// True for an opening bracket; returns its closer.
fn closer_of(p: Punct) -> Option<Punct> {
    match p {
        Punct::LParen => Some(Punct::RParen),
        Punct::LSquare => Some(Punct::RSquare),
        Punct::LBrace => Some(Punct::RBrace),
        _ => None,
    }
}

impl Parser<'_, '_> {
    // -- token capture --------------------------------------------------------------

    /// Consumes the current token into `out`.
    fn take_into(&mut self, out: &mut Vec<Token>) {
        let t = self.bump();
        out.push(t);
    }

    /// clang's `ConsumeAndStoreUntil`: moves tokens into `out` until one
    /// of `stops` (moved too when `consume_final`), balancing brackets.
    /// Returns false at end of input, at a `;` when `stop_at_semi`, or at
    /// a closer of a bracket opened before the capture began.
    pub(crate) fn capture_until(
        &mut self,
        stops: &[Punct],
        out: &mut Vec<Token>,
        stop_at_semi: bool,
        consume_final: bool,
    ) -> bool {
        let mut first = true;
        loop {
            let t = self.tok();
            let p = Self::punct_of(t);
            if let Some(p) = p
                && stops.contains(&p)
            {
                if consume_final {
                    self.take_into(out);
                }
                return true;
            }
            if t.kind == TokKind::Eof {
                return false;
            }
            match p {
                Some(open) if closer_of(open).is_some() => {
                    self.take_into(out);
                    if let Some(close) = closer_of(open) {
                        self.capture_until(&[close], out, false, true);
                    }
                }
                Some(close @ (Punct::RParen | Punct::RSquare | Punct::RBrace)) => {
                    if self.depth_of(close) > 0 && !first {
                        return false;
                    }
                    self.take_into(out);
                }
                Some(Punct::Semi) if stop_at_semi => return false,
                _ => self.take_into(out),
            }
            first = false;
        }
    }

    /// clang's `ConsumeAndStoreConditional`: at `?`, moves the
    /// conditional's middle operand and the `:` into `out`.
    fn capture_conditional(&mut self, out: &mut Vec<Token>) -> bool {
        self.take_into(out);
        while !self.at(Punct::Colon) {
            if !self.capture_until(&[Punct::Question, Punct::Colon], out, true, false) {
                return false;
            }
            if self.at(Punct::Question) && !self.capture_conditional(out) {
                return false;
            }
        }
        self.take_into(out);
        true
    }

    /// clang's `ConsumeAndStoreInitializer` (without its template-argument
    /// heuristics): moves a default argument or default member initializer
    /// into `out`, up to (not including) the `,`, `)` or `;` that ends it.
    /// Returns false at end of input or at an unbalanced closer.
    pub(crate) fn capture_initializer(&mut self, kind: InitKind, out: &mut Vec<Token>) -> bool {
        let mut first = true;
        loop {
            let t = self.tok();
            match Self::punct_of(t) {
                Some(Punct::Comma) => return true,
                Some(Punct::Question) => {
                    if !self.capture_conditional(out) {
                        return false;
                    }
                }
                Some(open @ (Punct::LParen | Punct::LSquare | Punct::LBrace)) => {
                    self.take_into(out);
                    if let Some(close) = closer_of(open) {
                        self.capture_until(&[close], out, true, true);
                    }
                }
                Some(Punct::RParen) if kind == InitKind::DefaultArg => return true,
                Some(close @ (Punct::RParen | Punct::RSquare | Punct::RBrace)) => {
                    if self.depth_of(close) > 0 && !first {
                        return false;
                    }
                    self.take_into(out);
                    // clang keeps `IsFirstToken` here (`continue`).
                    continue;
                }
                Some(Punct::Semi) if kind == InitKind::Nsdmi => return true,
                _ if t.kind == TokKind::Eof => return false,
                _ if self.kw_of(t) == Some(Kw::Operator) => {
                    // `operator,` and friends: the punctuator is a name.
                    self.take_into(out);
                    if matches!(
                        Self::punct_of(self.tok()),
                        Some(
                            Punct::Comma
                                | Punct::Greater
                                | Punct::GreaterGreater
                                | Punct::Less
                                | Punct::LessLess
                        )
                    ) {
                        self.take_into(out);
                    }
                }
                _ => self.take_into(out),
            }
            first = false;
        }
    }

    /// Saves a delayed default argument at `=` (spec/parse.md §2.4; clang's
    /// `ParseParameterDeclarationClause` in a member declarator). The
    /// sentinel sits at the token that ends it.
    pub(crate) fn capture_default_arg(&mut self) -> LateToks {
        let mut toks = Vec::new();
        self.take_into(&mut toks);
        self.capture_initializer(InitKind::DefaultArg, &mut toks);
        LateToks { toks, end: self.loc() }
    }

    /// Saves a delayed `noexcept ( ... )` / `throw ( ... )` at the keyword
    /// (clang's `tryParseExceptionSpecification` with `Delayed`).
    pub(crate) fn capture_exception_spec(&mut self) -> LateToks {
        let mut toks = Vec::new();
        self.take_into(&mut toks);
        self.take_into(&mut toks);
        self.capture_until(&[Punct::RParen], &mut toks, true, true);
        let end = toks.last().map_or(self.loc(), |t| t.span.hi);
        LateToks { toks, end }
    }

    /// clang's `ConsumeAndStoreFunctionPrologue`: moves an optional `try`,
    /// a ctor-initializer and the body's `{` into `out`. Returns false
    /// (reported) if no body was found.
    fn capture_function_prologue(&mut self, out: &mut Vec<Token>) -> bool {
        if self.at_kw(Kw::Try) {
            self.take_into(out);
        }
        if !self.at(Punct::Colon) {
            self.capture_until(&[Punct::LBrace, Punct::RBrace], out, true, false);
            if !self.at(Punct::LBrace) {
                self.expected_here("'{'");
                return false;
            }
            self.take_into(out);
            return true;
        }
        self.take_into(out);
        let mut might_be_template = false;
        loop {
            if self.at_kw(Kw::Decltype) {
                let open = self.loc();
                self.take_into(out);
                if !self.at(Punct::LParen) {
                    let d = self
                        .s
                        .diag(DiagId::parse_expected_after, self.loc())
                        .arg("'decltype'")
                        .arg("'('");
                    self.emit(d);
                    return false;
                }
                self.take_into(out);
                if !self.capture_until(&[Punct::RParen], out, true, true) {
                    self.expected_closer(Punct::RParen, Punct::LParen, open);
                    return false;
                }
            }
            loop {
                if self.at(Punct::ColonColon) {
                    self.take_into(out);
                    if self.at_kw(Kw::Template) {
                        self.take_into(out);
                    }
                }
                if self.ident().is_none() {
                    break;
                }
                self.take_into(out);
                if !self.at(Punct::ColonColon) {
                    break;
                }
            }
            if self.at(Punct::Comma) {
                // A missing initializer: diagnosed on replay.
                self.take_into(out);
                continue;
            }
            if self.at(Punct::Less) {
                might_be_template = true;
            }
            if might_be_template {
                if !self.capture_until(&[Punct::LParen, Punct::LBrace], out, true, false) {
                    self.expected_here("'{'");
                    return false;
                }
            } else if !self.at(Punct::LParen) && !self.at(Punct::LBrace) {
                self.expected_here("'(' or '{'");
                return false;
            }
            let is_paren = self.at(Punct::LParen);
            let open = self.loc();
            let prev = out.last().copied();
            self.take_into(out);
            if !is_paren && !might_be_template {
                let named = prev.is_some_and(|t| {
                    self.ident_of(t).is_some()
                        || matches!(Self::punct_of(t), Some(Punct::Greater | Punct::GreaterGreater))
                });
                if !named && self.brace_starts_body() {
                    // A malformed initializer before the body: the `{`
                    // is the body (diagnosed on replay).
                    return true;
                }
            }
            let (close, open_p) = if is_paren {
                (Punct::RParen, Punct::LParen)
            } else {
                (Punct::RBrace, Punct::LBrace)
            };
            if !self.capture_until(&[close], out, true, true) {
                self.expected_closer(close, open_p, open);
                return false;
            }
            if self.at(Punct::Ellipsis) {
                self.take_into(out);
            }
            if self.at(Punct::Comma) {
                self.take_into(out);
            } else if self.at(Punct::LBrace) {
                self.take_into(out);
                return true;
            } else if !might_be_template {
                self.expected_here("'{' or ','");
                return false;
            }
        }
    }

    /// After a `{` with no mem-initializer-id before it: true if the token
    /// after its matching `}` is not `,`, `...` or `{` (so the `{` is more
    /// likely the function body; clang's tentative `SkipUntil`).
    fn brace_starts_body(&self) -> bool {
        let mut depth = 1u32;
        let mut i = self.pos();
        loop {
            let t = self.tok_at(i);
            match Self::punct_of(t) {
                _ if t.kind == TokKind::Eof => return false,
                Some(Punct::LBrace) => depth += 1,
                Some(Punct::RBrace) => {
                    depth -= 1;
                    if depth == 0 {
                        return !matches!(
                            Self::punct_of(self.tok_at(i + 1)),
                            Some(Punct::Comma | Punct::Ellipsis | Punct::LBrace)
                        );
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }

    /// Reports `expected {what}` at the current token.
    fn expected_here(&self, what: &str) {
        let d = self.s.diag(DiagId::parse_expected, self.loc()).arg(what);
        self.emit(d);
    }

    /// Reports a missing `close` at the current token, with a note at the
    /// `open` at `open_loc`.
    fn expected_closer(&self, close: Punct, open: Punct, open_loc: Loc) {
        let note = self.s.diag(DiagId::note_matching, open_loc).arg(quote(open));
        let d = self.s.diag(DiagId::parse_expected, self.loc()).arg(quote(close)).with_note(note);
        self.emit(d);
    }

    // -- skipping -------------------------------------------------------------------

    /// Skips a function body (an optional `try`, a ctor-initializer, the
    /// compound statement and handlers) that will not be parsed.
    pub(crate) fn skip_function_body(&mut self) {
        let is_try = self.eat_kw(Kw::Try).is_some();
        if self.at(Punct::Colon) {
            self.skip_until(&[Punct::LBrace], true, false);
        }
        if self.at(Punct::LBrace) {
            self.bump();
            self.skip_until(&[Punct::RBrace], false, true);
        }
        if is_try {
            while self.eat_kw(Kw::Catch).is_some() {
                if self.at(Punct::LParen) {
                    self.bump();
                    self.skip_until(&[Punct::RParen], false, true);
                }
                if self.at(Punct::LBrace) {
                    self.bump();
                    self.skip_until(&[Punct::RBrace], false, true);
                }
            }
        }
    }

    /// clang's `SkipDeletedFunctionBody`: after `= delete` where it is not
    /// allowed, skips a following `( message )`.
    pub(crate) fn skip_deleted_function_body(&mut self) {
        if self.at(Punct::LParen) {
            self.bump();
            self.skip_until(&[Punct::RParen, Punct::Comma], true, false);
            self.eat(Punct::RParen);
        }
    }

    /// clang's `SkipMalformedDecl`: skips to the end of a broken member
    /// declaration (through a `;` or a function body, or up to a `}`).
    fn skip_malformed_decl(&mut self) {
        loop {
            match Self::punct_of(self.tok()) {
                Some(Punct::LBrace) => {
                    self.bump();
                    self.skip_until(&[Punct::RBrace], false, true);
                    if self.at(Punct::Comma) || self.at(Punct::LBrace) || self.at_kw(Kw::Try) {
                        continue;
                    }
                    self.eat(Punct::Semi);
                    return;
                }
                Some(Punct::LParen | Punct::LSquare) => {
                    let close = if self.at(Punct::LParen) { Punct::RParen } else { Punct::RSquare };
                    self.bump();
                    self.skip_until(&[close], false, true);
                }
                Some(Punct::RBrace) => return,
                Some(Punct::Semi) => {
                    self.bump();
                    return;
                }
                _ if self.at_eof() => return,
                _ => {
                    self.bump();
                }
            }
        }
    }

    // -- class bodies -----------------------------------------------------------------

    /// The name of the innermost class being defined (for constructor
    /// declarators, [class.ctor.general]/1).
    pub(crate) fn current_class_name(&self) -> Option<Symbol> {
        let r = self.late.last()?.record;
        self.s.ast.decls.get(r).name().as_ident()
    }

    /// The spelling of a declaration's name, for traces.
    fn decl_name(&self, d: DeclId) -> String {
        self.s
            .ast
            .decls
            .get(d)
            .name()
            .as_ident()
            .map_or_else(|| "<anon>".to_owned(), |n| self.s.ast.symbols().as_str(n).to_owned())
    }

    /// The base-clause and `{ member-specification }` of class `r`
    /// ([class.pre], [class.mem]; spec/sema-class.md §2.1), at `:` or
    /// `{`; `final_loc` is its class-virt-specifier. For the outermost
    /// class, then replays the saved complete-class contexts.
    pub(crate) fn parse_record_body(&mut self, r: DeclId, final_loc: Option<Loc>) {
        let saved = mem::take(&mut self.late_fn);
        let nested = !self.late.is_empty();
        self.late.push(LateClass { record: r, items: Vec::new() });
        self.trace(|p| format!("start_record_definition({})", p.decl_name(r)));
        self.s.act_on_start_record_definition(r);
        if let Some(l) = final_loc {
            self.trace(|_| "class_final".to_owned());
            self.s.act_on_class_final(r, l);
        }
        let mut bases = Vec::new();
        if self.at(Punct::Colon) {
            self.parse_base_clause(&mut bases);
        }
        self.trace(|_| format!("base_specifiers({})", bases.len()));
        self.s.act_on_base_specifiers(r, &bases);
        if self.at(Punct::LBrace) {
            let open = self.bump().span.lo;
            while !self.at(Punct::RBrace) && !self.at_eof() {
                self.parse_member_declaration();
                self.s.act_on_end_abbreviated();
            }
            self.expect_closing(Punct::RBrace, Punct::LBrace, open);
        } else {
            self.s.error(DiagId::parse_expected_lbrace_after_base, self.prev_end());
        }
        let then_semi = self.at(Punct::Semi);
        self.trace(|p| {
            format!(
                "finish_record_definition({}, {})",
                p.decl_name(r),
                if then_semi { "semi" } else { "nosemi" }
            )
        });
        self.s.act_on_finish_record_definition(r, then_semi);
        let class = self.late.pop();
        if let Some(mut class) = class {
            if nested {
                if !class.items.is_empty()
                    && let Some(parent) = self.late.last_mut()
                {
                    parent.items.push(LateItem::Nested(class));
                }
            } else {
                for ph in [Phase::Method, Phase::Nsdmi, Phase::Body] {
                    self.replay_phase(&mut class, ph);
                }
                self.trace(|p| format!("finish_class_contexts({})", p.decl_name(r)));
                self.s.act_on_finish_class_contexts(r);
            }
        }
        self.late_fn = saved;
    }

    /// Replays the items of phase `ph` of `class` and its nested classes,
    /// in declaration order, each class's inside its re-entered scope
    /// (clang's `ParseLexedMethodDeclarations` / `ParseLexedMemberInitializers`
    /// / `ParseLexedMethodDefs`).
    fn replay_phase(&mut self, class: &mut LateClass, ph: Phase) {
        if !class.has(ph) {
            return;
        }
        let r = class.record;
        self.trace(|p| format!("start_delayed_class({})", p.decl_name(r)));
        self.s.act_on_start_delayed_class(r);
        for item in &mut class.items {
            match item {
                LateItem::Method { f, parts } if ph == Phase::Method => {
                    let parts = mem::take(parts);
                    let n = self.s.act_on_reenter_template_scopes(*f);
                    self.replay_method(*f, parts);
                    self.s.act_on_exit_reentered_template_scopes(n);
                }
                LateItem::Nsdmi { field, toks } if ph == Phase::Nsdmi => {
                    let t = mem::take(&mut toks.toks);
                    self.replay_nsdmi(*field, t, toks.end);
                }
                LateItem::Body { f, ctor, toks } if ph == Phase::Body => {
                    let t = mem::take(&mut toks.toks);
                    self.begin_replay(t, toks.end);
                    let n = self.s.act_on_reenter_template_scopes(*f);
                    self.parse_function_definition(*f, *ctor);
                    self.s.act_on_exit_reentered_template_scopes(n);
                    self.finish_replay();
                }
                LateItem::Nested(c) => self.replay_phase(c, ph),
                _ => {}
            }
        }
        self.trace(|p| format!("finish_delayed_class({})", p.decl_name(r)));
        self.s.act_on_finish_delayed_class(r);
    }

    /// Skips what is left of the saved tokens and ends the replay.
    fn finish_replay(&mut self) {
        while !self.at_eof() {
            self.bump();
        }
        self.end_replay();
    }

    /// The default arguments and *noexcept-specifier* of `f`
    /// (clang's `ParseLexedMethodDeclaration`).
    fn replay_method(&mut self, f: DeclId, parts: LateFnParts) {
        self.trace(|p| format!("start_delayed_method({})", p.decl_name(f)));
        self.s.act_on_start_delayed_method(f);
        let params: Vec<DeclId> = self
            .s
            .ast
            .decls
            .function(f)
            .map(|x| self.s.ast.lists.iter::<DeclId>(x.params).collect())
            .unwrap_or_default();
        for (i, t) in parts.default_args {
            self.begin_replay(t.toks, t.end);
            self.bump();
            if self.at(Punct::LBrace) {
                self.unsupported("braced default arguments", self.loc());
            } else if let Some(e) = self.parse_assignment_expr() {
                if !self.at_eof() {
                    self.error_here(DiagId::parse_default_arg_unparsed);
                }
                if let Some(&p) = params.get(i) {
                    self.trace(|_| format!("default_arg({i})"));
                    self.s.act_on_param_default_arg(p, e);
                }
            }
            self.finish_replay();
        }
        if let Some(t) = parts.noexcept {
            self.begin_replay(t.toks, t.end);
            let spec = self.parse_exception_spec();
            if !self.at_eof() {
                self.error_here(DiagId::parse_except_spec_unparsed);
            }
            self.trace(|_| "exception_spec".to_owned());
            self.s.act_on_exception_spec(f, spec);
            self.finish_replay();
        }
        self.trace(|p| format!("finish_delayed_method({})", p.decl_name(f)));
        self.s.act_on_finish_delayed_method(f);
    }

    /// The default member initializer of `field` (clang's
    /// `ParseLexedMemberInitializer`).
    fn replay_nsdmi(&mut self, field: DeclId, toks: Vec<Token>, end: Loc) {
        if self.s.ast.decls.get(field).has(DeclFlags::INVALID) {
            return;
        }
        self.begin_replay(toks, end);
        self.trace(|p| format!("start_field_init({})", p.decl_name(field)));
        self.s.act_on_start_field_init(field);
        if let Some(init) = self.parse_member_initializer(false) {
            self.trace(|p| format!("field_init({})", p.decl_name(field)));
            self.s.act_on_field_init(field, init);
            if !self.at_eof() {
                self.s.error(DiagId::parse_expected_semi_decl_list, self.prev_end());
            }
        }
        self.finish_replay();
    }

    /// brace-or-equal-initializer of a member declarator (clang's
    /// `ParseCXXMemberInitializer`), at `=` or `{`. `= delete` and
    /// `= default` are reported here: they can only define the first
    /// declarator of a function.
    fn parse_member_initializer(&mut self, is_fn: bool) -> Option<Initializer> {
        if self.at(Punct::Equal) {
            self.bump();
            if self.at_kw(Kw::Delete)
                && (is_fn
                    || matches!(Self::punct_of(self.peek(1)), Some(Punct::Semi | Punct::Comma))
                    || self.peek(1).kind == TokKind::Eof)
            {
                let l = self.bump().span.lo;
                if is_fn {
                    let d = self
                        .s
                        .diag(DiagId::parse_default_delete_in_multiple_declaration, l)
                        .arg("delete");
                    self.emit(d);
                } else {
                    self.s.error(DiagId::parse_deleted_non_function, l);
                }
                self.skip_deleted_function_body();
                return None;
            }
            if self.at_kw(Kw::Default) {
                if is_fn {
                    let d = self
                        .s
                        .diag(DiagId::parse_default_delete_in_multiple_declaration, self.loc())
                        .arg("default");
                    self.emit(d);
                } else {
                    let l = self.bump().span.lo;
                    self.s.error(DiagId::parse_default_special_members, l);
                }
                return None;
            }
        }
        if self.at(Punct::LBrace) {
            self.parse_init_list().ok().map(Initializer::List)
        } else {
            self.parse_assignment_expr().map(Initializer::Expr)
        }
    }

    // -- function definitions -----------------------------------------------------------

    /// The function-body of the function definition `f` ([dcl.fct.def.general],
    /// [class.base.init]), at `:`, `try` or `{`; `ctor` if `f` declares a
    /// constructor.
    pub(crate) fn parse_function_definition(&mut self, f: DeclId, ctor: bool) {
        if self.at_kw(Kw::Try) {
            self.unsupported("function try blocks", self.loc());
            self.skip_function_body();
            return;
        }
        self.trace(|p| format!("start_function_def({})", p.decl_name(f)));
        self.s.act_on_start_function_def(f);
        let had_colon = self.at(Punct::Colon);
        if had_colon {
            let colon = self.bump().span.lo;
            let mut count = 0usize;
            loop {
                let (info, ok) = self.parse_mem_initializer();
                // clang's sema rejects every mem-initializer of a
                // non-constructor (reported once, below).
                let valid = ok && ctor;
                // Each mem-initializer is a full-expression, built as soon
                // as it is parsed ([class.base.init]/7).
                if let Some(info) = info
                    && ctor
                {
                    count += 1;
                    self.s.act_on_mem_initializer(f, &info);
                }
                if self.eat(Punct::Comma).is_some() {
                    continue;
                }
                if self.at(Punct::LBrace) {
                    break;
                }
                if valid && (self.ident().is_some() || self.at(Punct::ColonColon)) {
                    self.s.error(DiagId::parse_ctor_init_missing_comma, self.prev_end());
                    continue;
                }
                if valid {
                    self.expected_here("'{' or ','");
                }
                self.skip_until(&[Punct::LBrace], true, false);
                break;
            }
            if ctor {
                self.trace(|_| format!("mem_initializers({count})"));
                self.s.act_on_mem_initializers(f, &[], colon);
            } else {
                self.s.error(DiagId::parse_only_constructors_take_base_inits, colon);
            }
        } else if ctor {
            self.trace(|_| "mem_initializers(0)".to_owned());
            self.s.act_on_mem_initializers(f, &[], self.loc());
        }
        if !self.at(Punct::LBrace) {
            if !had_colon {
                self.expected_here("'{'");
            }
            let l = self.loc();
            let body = self.s.act_on_compound(&[], l, l);
            self.trace(|p| format!("finish_function_def({})", p.decl_name(f)));
            self.s.act_on_finish_function_def(f, body, l);
            return;
        }
        let (body, rbrace) = self.parse_compound(false);
        self.trace(|p| format!("finish_function_def({})", p.decl_name(f)));
        self.s.act_on_finish_function_def(f, body, rbrace);
    }

    /// mem-initializer ([class.base.init]): the initializer (when sema can
    /// take it) and whether it parsed.
    fn parse_mem_initializer(&mut self) -> (Option<MemInitInfo>, bool) {
        let Ok(q) = self.parse_nested_name() else {
            return (None, false);
        };
        let loc = self.loc();
        if self.at_kw(Kw::Decltype) {
            self.unsupported("'decltype'", loc);
            self.bump();
            if self.at(Punct::LParen) {
                self.bump();
                self.skip_until(&[Punct::RParen], false, true);
            }
            let ok = self.parse_mem_init_args().is_some();
            self.eat(Punct::Ellipsis);
            return (None, ok);
        }
        let Some(name) = self.ident() else {
            self.error_here(DiagId::parse_expected_member_or_base_name);
            return (None, false);
        };
        self.bump();
        let Some(init) = self.parse_mem_init_args() else {
            return (None, false);
        };
        let pack_expansion = self.eat(Punct::Ellipsis).is_some();
        let id = match q {
            None => MemInitId::Ident(name),
            Some(q) => {
                if let NameClass::Type(ty, d) = self.s.classify_name(name, Some(q)) {
                    MemInitId::Type(self.named_type(ty, d))
                } else {
                    self.unsupported("qualified member initializers", loc);
                    return (None, true);
                }
            }
        };
        (Some(MemInitInfo { id, loc, init, pack_expansion }), true)
    }

    /// The `( expression-list )` or braced-init-list of a mem-initializer.
    fn parse_mem_init_args(&mut self) -> Option<Initializer> {
        if self.at(Punct::LBrace) {
            return self.parse_init_list().ok().map(Initializer::List);
        }
        if self.at(Punct::LParen) {
            return self.parse_initializer(Some(InitStyle::Call), false).ok().flatten();
        }
        self.expected_here("'(' or '{'");
        None
    }

    /// The type a class-name names, as a type-specifier would write it.
    fn named_type(&mut self, ty: QualType, d: DeclId) -> QualType {
        self.s.mark_type_referenced(d);
        if kiln_sema::lookup::is_tag_kind(self.s.ast.decls.get(d).kind()) {
            self.s.elaborated_tag_type(d, TagKeyword::None)
        } else {
            ty
        }
    }

    /// `= default ;` / `= delete ;` / `= delete ( string-literal ) ;` after
    /// the first declarator of a function ([dcl.fct.def.general]), at `=`;
    /// `f` is the function (`None` when sema made none).
    pub(crate) fn parse_defaulted_or_deleted(&mut self, f: Option<DeclId>) {
        self.bump();
        let kw = self.bump();
        let is_delete = self.kw_of(kw) == Some(Kw::Delete);
        let what = if is_delete { "delete" } else { "default" };
        let mut msg = None;
        if is_delete && self.at(Punct::LParen) {
            let open = self.bump().span.lo;
            if self.at_string() {
                msg = self.parse_string_literal(true);
            } else {
                let d = self
                    .s
                    .diag(DiagId::parse_expected_string_literal_in, self.loc())
                    .arg("'delete'");
                self.emit(d);
                self.skip_until(&[Punct::RParen], true, false);
            }
            self.expect_closing(Punct::RParen, Punct::LParen, open);
        }
        if let Some(f) = f {
            if is_delete {
                self.trace(|_| "deleted".to_owned());
                self.s.act_on_deleted(f, kw.span.lo, msg);
            } else {
                self.trace(|_| "defaulted".to_owned());
                self.s.act_on_defaulted(f, kw.span.lo);
            }
        }
        match Self::punct_of(self.tok()) {
            Some(Punct::Comma) => {
                let d = self
                    .s
                    .diag(DiagId::parse_default_delete_in_multiple_declaration, kw.span.lo)
                    .arg(what);
                self.emit(d);
                self.skip_until(&[Punct::Semi], false, true);
            }
            Some(Punct::Semi) => {
                self.bump();
            }
            Some(Punct::Colon) => {
                let d = self.s.diag(DiagId::parse_expected_after, self.loc()).arg(what).arg("';'");
                self.emit(d);
                self.bump();
            }
            _ => {
                let d =
                    self.s.diag(DiagId::parse_expected_after, self.prev_end()).arg(what).arg("';'");
                self.emit(d);
                self.skip_until(&[Punct::Semi], false, true);
            }
        }
    }

    // -- member declarations ------------------------------------------------------------

    /// The member-declarations that a leading token decides (empty
    /// declarations, access specifiers, `static_assert`, `using`,
    /// templates): parses one and returns true, or returns false.
    fn parse_member_declaration_leader(&mut self) -> bool {
        let loc = self.loc();
        match (Self::punct_of(self.tok()), self.kw()) {
            (Some(Punct::Semi), _) => {
                self.bump();
                true
            }
            (_, Some(k @ (Kw::Public | Kw::Private | Kw::Protected))) => {
                let access = match k {
                    Kw::Public => Access::Public,
                    Kw::Protected => Access::Protected,
                    _ => Access::Private,
                };
                self.bump();
                if self.eat(Punct::Colon).is_none() {
                    if self.at(Punct::Semi) {
                        self.expected_here("':'");
                        self.bump();
                    } else {
                        let d = self.s.diag(DiagId::parse_expected, self.prev_end()).arg("':'");
                        self.emit(d);
                    }
                }
                self.trace(|_| "access_spec".to_owned());
                let _ = self.s.act_on_access_spec(access, loc);
                true
            }
            (_, Some(Kw::StaticAssert)) => {
                self.parse_static_assert();
                true
            }
            (_, Some(Kw::Using)) => {
                self.parse_using();
                true
            }
            (Some(Punct::LSquare), _) if self.at_attributed_using() => {
                self.parse_attributed_using();
                true
            }
            (_, Some(Kw::Template)) => {
                self.parse_template_declaration(DeclaratorContext::Member);
                self.s.act_on_end_abbreviated();
                true
            }
            _ => false,
        }
    }

    /// The part of a member-declarator after the declarator itself, up to
    /// its initializer ([class.mem.general]; clang's
    /// `ParseCXXMemberDeclaratorBeforeInitializer`): a bit-width or
    /// virt-specifiers, an asm label and attributes. Returns whether a
    /// bit-width was present and whether it failed to parse.
    fn parse_member_declarator_tail(&mut self, d: &mut Declarator, friend: bool) -> (bool, bool) {
        let mut has_width = false;
        let mut width_failed = false;
        if !d.is_function_declarator() && self.at(Punct::Colon) {
            self.bump();
            has_width = true;
            if let Some(w) = self.parse_constant_expr() {
                d.bit_width = Some(w);
            } else {
                width_failed = true;
                self.skip_until(&[Punct::Comma], true, false);
            }
        } else {
            self.parse_virt_specifiers(d, friend);
        }
        match self.parse_asm_label() {
            Ok(l) => d.asm_label = l,
            Err(()) => {
                self.skip_until(&[Punct::Comma], true, false);
            }
        }
        if self.at_attr() {
            self.parse_attributes(&mut d.attrs);
        }
        if !has_width && d.virt_final.is_none() && d.virt_override.is_none() {
            self.parse_virt_specifiers(d, friend);
        }
        (has_width, width_failed)
    }

    /// Saves a default member initializer (`= x` or `{x}`) for replay
    /// after the outermost class ([class.mem.general]/7, spec/parse.md
    /// §4.1).
    fn save_nsdmi(&mut self, decl: Option<DeclId>) {
        let mut toks = Vec::new();
        if self.at(Punct::Equal) {
            self.take_into(&mut toks);
            self.capture_initializer(InitKind::Nsdmi, &mut toks);
        } else {
            self.take_into(&mut toks);
            self.capture_until(&[Punct::RBrace], &mut toks, true, true);
        }
        let end = self.loc();
        if let Some(field) = decl
            && self.s.ast.decls.get(field).kind() == DeclKind::Field
        {
            self.push_late(LateItem::Nsdmi { field, toks: LateToks { toks, end } });
        }
    }

    /// A member initializer parsed in place: a static data member's
    /// ([class.static.data]/3) or a function's non-pure `= x`. With no
    /// initializer, a static data member is uninitialized.
    fn parse_member_init_in_place(
        &mut self,
        d: &Declarator,
        decl: Option<DeclId>,
        init: bool,
        is_static: bool,
    ) {
        let is_fn = d.is_function_declarator();
        let var = decl.filter(|&v| self.s.ast.decls.get(v).kind() == DeclKind::Var);
        if !init {
            if is_static && let Some(v) = var {
                self.s.act_on_uninit(v);
            }
            return;
        }
        match self.parse_member_initializer(is_fn) {
            Some(_) if is_fn => {
                if !d.invalid {
                    self.s.error(DiagId::parse_bad_pure_specifier, d.name_loc());
                }
            }
            Some(i) => {
                if let Some(v) = var {
                    self.trace(|_| "init".to_owned());
                    self.s.act_on_init(v, i);
                }
            }
            None => {
                if let Some(v) = var {
                    self.s.act_on_uninit(v);
                }
                self.skip_until(&[Punct::Comma], true, false);
            }
        }
    }

    /// The type named by a member declaration's decl-specifiers, formed
    /// once the first declarator is known: clang reports a missing type
    /// specifier at its name.
    fn member_base_type(&mut self, ds: &DeclSpec, d: &Declarator) -> QualType {
        if !ds.has_type_specifier() && is_untyped_special(d) {
            QualType::VOID
        } else if d.invalid && !ds.has_type_specifier() {
            // A malformed special declarator-id (`operator;`) is reported;
            // clang adds nothing about the type.
            QualType::ERROR
        } else {
            self.s.type_from_declspec_at(ds, d.name_loc())
        }
    }

    /// `decl-specifier-seq ;` in a class: a friend class declaration or a
    /// declaration with no declarator ([class.mem.general]/3).
    fn act_on_member_decl_spec_only(&mut self, ds: &DeclSpec) {
        if let Some(fl) = ds.friend {
            self.trace(|_| "friend".to_owned());
            let base = self.s.type_from_declspec(ds);
            let _ = self.s.act_on_friend(ds, base, None, fl);
        } else {
            self.trace(|_| "decl_spec_only".to_owned());
            let _ = self.s.act_on_decl_spec_only(ds);
        }
    }

    /// member-declaration ([class.mem]; clang's
    /// `ParseCXXClassMemberDeclaration`).
    pub(crate) fn parse_member_declaration(&mut self) {
        if self.parse_member_declaration_leader() {
            return;
        }
        let ds = self.parse_decl_specifiers(DeclaratorContext::Member);
        if self.at(Punct::Semi) {
            self.bump();
            self.act_on_member_decl_spec_only(&ds);
            return;
        }
        if ds.is_empty() && !self.at_special_declarator(DeclaratorContext::Member) {
            self.error_here(DiagId::parse_expected_member_name);
            self.skip_to_semi();
            return;
        }
        let is_static = matches!(ds.storage, Some((StorageSpec::Static, _)));
        let is_typedef = ds.is_typedef();
        let mut base_ty: Option<QualType> = None;
        let mut first = true;
        loop {
            let mut d = Declarator::new(DeclaratorContext::Member, self.loc());
            self.late_fn = LateFnParts {
                delay_args: !is_typedef,
                delay_noexcept: !is_typedef && ds.friend.is_none(),
                ..LateFnParts::default()
            };
            if !self.at(Punct::Colon) {
                self.parse_declarator(&mut d);
                self.parse_trailing_requires(&mut d);
            }
            let parts = mem::take(&mut self.late_fn);
            let base = *base_ty.get_or_insert_with(|| self.member_base_type(&ds, &d));
            let is_fn = d.is_function_declarator();
            let (has_width, width_failed) =
                self.parse_member_declarator_tail(&mut d, ds.friend.is_some());
            if !d.has_name() && !has_width {
                if !d.invalid {
                    self.error_here(DiagId::parse_expected_member_name);
                }
                self.skip_until(&[Punct::RBrace], true, false);
                if first {
                    self.eat(Punct::Semi);
                    return;
                }
                break;
            }
            if first && is_fn && self.at_function_definition() {
                self.parse_member_function_definition(&ds, base, &d, parts);
                return;
            }
            // clang's in-class initializer classification.
            let mut nsdmi = false;
            let mut init = false;
            if self.at(Punct::Equal) || self.at(Punct::LBrace) {
                if has_width && !d.has_name() {
                    self.error_here(DiagId::parse_anon_bitfield_member_init);
                    self.skip_until(&[Punct::Comma], true, false);
                } else if is_fn {
                    if let Some(z) = self.try_pure_specifier() {
                        d.pure = Some(z);
                    } else {
                        init = true;
                    }
                } else {
                    d.init_style =
                        Some(if self.at(Punct::Equal) { InitStyle::C } else { InitStyle::List });
                    if !is_static && !is_typedef && ds.friend.is_none() {
                        nsdmi = true;
                    } else {
                        init = true;
                    }
                }
            }
            let (decl, target) =
                if width_failed { (None, None) } else { self.act_on_member(&ds, base, &d) };
            if nsdmi {
                self.save_nsdmi(decl);
            } else {
                self.parse_member_init_in_place(&d, decl, init, is_static);
            }
            if !parts.is_empty()
                && let Some(f) = target
            {
                self.push_late(LateItem::Method { f, parts });
            }
            if self.eat(Punct::Comma).is_none() {
                break;
            }
            first = false;
        }
        if self.eat(Punct::Semi).is_none() {
            self.s.error(DiagId::parse_expected_semi_decl_list, self.expected_loc());
            self.skip_until(&[Punct::RBrace], true, false);
            self.eat(Punct::Semi);
        }
    }

    /// True at the start of a function definition after a declarator:
    /// `{`, `:`, `try`, `= default` or `= delete`.
    fn at_function_definition(&self) -> bool {
        self.at(Punct::LBrace)
            || self.at(Punct::Colon)
            || self.at_kw(Kw::Try)
            || (self.at(Punct::Equal)
                && matches!(self.kw_of(self.peek(1)), Some(Kw::Default | Kw::Delete)))
    }

    /// A member function definition in a class (clang's
    /// `ParseCXXInlineMethodDef`): declares it, then saves its body for
    /// the replay after the outermost class.
    fn parse_member_function_definition(
        &mut self,
        ds: &DeclSpec,
        base: QualType,
        d: &Declarator,
        parts: LateFnParts,
    ) {
        let (_, target) = self.act_on_member(ds, base, d);
        if !parts.is_empty()
            && let Some(f) = target
        {
            self.push_late(LateItem::Method { f, parts });
        }
        if self.at(Punct::Equal) {
            self.parse_defaulted_or_deleted(target);
            return;
        }
        let ctor = matches!(d.special, Some((SpecialName::Constructor(_), _)));
        let is_try = self.at_kw(Kw::Try);
        let mut toks = Vec::new();
        if !self.capture_function_prologue(&mut toks) {
            self.skip_malformed_decl();
            return;
        }
        self.capture_until(&[Punct::RBrace], &mut toks, false, true);
        if is_try {
            while self.at_kw(Kw::Catch) {
                self.capture_until(&[Punct::LBrace], &mut toks, false, true);
                self.capture_until(&[Punct::RBrace], &mut toks, false, true);
            }
        }
        if let Some(f) = target {
            let end = toks.last().map_or(self.loc(), |t| t.span.hi);
            self.push_late(LateItem::Body { f, ctor, toks: LateToks { toks, end } });
        }
        self.eat(Punct::Semi);
    }

    /// Declares a member declarator (or a friend function): returns the
    /// declaration and the function it declares or befriends.
    fn act_on_member(
        &mut self,
        ds: &DeclSpec,
        base: QualType,
        d: &Declarator,
    ) -> (Option<DeclId>, Option<DeclId>) {
        let decl = if let Some(fl) = ds.friend {
            self.trace(|_| "friend".to_owned());
            self.s.act_on_friend(ds, base, Some(d), fl)
        } else {
            self.trace(|p| format!("declarator({})", p.declarator_name(d)));
            self.s.act_on_declarator(ds, base, d)
        };
        let target = decl.and_then(|x| {
            let decls = &self.s.ast.decls;
            if decls.get(x).kind() == DeclKind::Friend {
                match decls.friend_target(x) {
                    FriendTarget::Decl(t) if decls.function(t).is_some() => Some(t),
                    _ => None,
                }
            } else {
                decls.function(x).map(|_| x)
            }
        });
        (decl, target)
    }

    /// The declarator's name, for traces.
    fn declarator_name(&self, d: &Declarator) -> String {
        let sym = |s: Symbol| self.s.ast.symbols().as_str(s).to_owned();
        match (d.name, &d.special) {
            (Some((n, _)), _) => sym(n),
            (None, Some((SpecialName::Constructor(n), _))) => sym(*n),
            (None, Some((SpecialName::Destructor(n), _))) => format!("~{}", sym(*n)),
            (None, Some((SpecialName::Conversion(_), _))) => "operator <conv>".to_owned(),
            (None, Some((SpecialName::Operator(_), _))) => "operator".to_owned(),
            (None, None) => "<anon>".to_owned(),
        }
    }

    /// Adds a saved complete-class context to the innermost class.
    fn push_late(&mut self, item: LateItem) {
        if let Some(c) = self.late.last_mut() {
            c.items.push(item);
        }
    }

    /// pure-specifier `= 0` before `;` or `,` ([class.mem.general];
    /// clang's `TryConsumePureSpecifier`): consumes it and returns the
    /// `0`'s location.
    fn try_pure_specifier(&mut self) -> Option<Loc> {
        if !self.at(Punct::Equal) {
            return None;
        }
        let zero = self.peek(1);
        if zero.kind != TokKind::PpNumber
            || self.spelling_of(&zero) != "0"
            || !matches!(Self::punct_of(self.peek(2)), Some(Punct::Semi | Punct::Comma))
        {
            return None;
        }
        self.bump();
        self.bump();
        Some(zero.span.lo)
    }

    /// virt-specifier-seq ([class.mem.general]; clang's
    /// `ParseOptionalCXX11VirtSpecifierSeq`).
    fn parse_virt_specifiers(&mut self, d: &mut Declarator, is_friend: bool) {
        loop {
            let t = self.tok();
            let (is_final, sp) = if self.is_final_tok(t) {
                (true, "final")
            } else if self.is_override_tok(t) {
                (false, "override")
            } else {
                return;
            };
            let loc = self.bump().span.lo;
            if is_friend {
                let diag = self.s.diag(DiagId::parse_friend_decl_spec, loc).arg(sp);
                self.emit(diag);
                continue;
            }
            let slot = if is_final { &mut d.virt_final } else { &mut d.virt_override };
            if slot.is_some() {
                let diag = self.s.diag(DiagId::parse_duplicate_virt_specifier, loc).arg(sp);
                self.emit(diag);
            } else {
                *slot = Some(loc);
            }
        }
    }

    /// True if a declarator naming a constructor, destructor, conversion
    /// function or operator starts here, so that a decl-specifier-seq may
    /// be empty ([class.ctor.general]/1, [class.dtor]/1, [class.conv.fct];
    /// clang's `isConstructorDeclarator` and friends). Unqualified
    /// constructors only in the class's own member-specification.
    pub(crate) fn at_special_declarator(&self, ctx: DeclaratorContext) -> bool {
        if !matches!(ctx, DeclaratorContext::File | DeclaratorContext::Member) {
            return false;
        }
        let mut i = self.pos();
        if Self::punct_of(self.tok_at(i)) == Some(Punct::ColonColon) {
            i += 1;
        }
        let mut last = None;
        while let Some(n) = self.ident_of(self.tok_at(i))
            && Self::punct_of(self.tok_at(i + 1)) == Some(Punct::ColonColon)
        {
            last = Some(n);
            i += 2;
        }
        let t = self.tok_at(i);
        if Self::punct_of(t) == Some(Punct::Tilde) || self.kw_of(t) == Some(Kw::Operator) {
            return true;
        }
        let Some(n) = self.ident_of(t) else {
            return false;
        };
        if Self::punct_of(self.tok_at(i + 1)) != Some(Punct::LParen) {
            return false;
        }
        match last {
            Some(q) => q == n,
            None => {
                ctx == DeclaratorContext::Member
                    && self.current_class_name() == Some(n)
                    && !matches!(
                        Self::punct_of(self.tok_at(i + 2)),
                        Some(Punct::Star | Punct::Amp | Punct::AmpAmp)
                    )
            }
        }
    }

    // -- special declarator-ids ----------------------------------------------------------

    /// The last name in the nested-name-specifier starting here (`B` in
    /// `A::B::f`), for recognising `C::C` ([class.ctor.general]/1).
    pub(crate) fn last_qualifier_name(&self) -> Option<Symbol> {
        let mut i = self.pos();
        if Self::punct_of(self.tok_at(i)) == Some(Punct::ColonColon) {
            i += 1;
        }
        let mut last = None;
        while let Some(n) = self.ident_of(self.tok_at(i))
            && Self::punct_of(self.tok_at(i + 1)) == Some(Punct::ColonColon)
        {
            last = Some(n);
            i += 2;
        }
        last
    }

    /// True if identifier `n`, which is at the current token, names a
    /// constructor: `C::C(` after a qualifier ending in `C`, or `C(` in
    /// the member-specification of class `C` (clang's
    /// `isConstructorDeclarator`, simplified: `C (*p)` and `C (&r)` are
    /// data members).
    pub(crate) fn names_constructor(
        &self,
        n: Symbol,
        last_qual: Option<Symbol>,
        ctx: DeclaratorContext,
    ) -> bool {
        if Self::punct_of(self.peek(1)) != Some(Punct::LParen) {
            return false;
        }
        match last_qual {
            Some(q) => q == n,
            None => {
                ctx == DeclaratorContext::Member
                    && self.current_class_name() == Some(n)
                    && !matches!(
                        Self::punct_of(self.peek(2)),
                        Some(Punct::Star | Punct::Amp | Punct::AmpAmp)
                    )
            }
        }
    }

    /// A destructor or operator-function-id / conversion-function-id
    /// declarator-id ([class.dtor]/1, [over.oper.general]/1,
    /// [class.conv.fct]/1), at `~` or `operator`. Sets `d.special`.
    pub(crate) fn parse_special_declarator_id(&mut self, d: &mut Declarator) {
        let loc = self.loc();
        if self.at(Punct::Tilde) {
            self.bump();
            if let Some(n) = self.ident() {
                self.bump();
                d.special = Some((SpecialName::Destructor(n), loc));
            } else {
                self.error_here(DiagId::parse_destructor_tilde_identifier);
                d.invalid = true;
            }
            return;
        }
        self.bump();
        if let Some(op) = self.parse_operator_symbol() {
            d.special = Some((SpecialName::Operator(op), loc));
            return;
        }
        if matches!(self.tok().kind, TokKind::StringLit(_)) {
            self.unsupported("literal operators", loc);
            self.bump();
            if self.ident().is_some() {
                self.bump();
            }
            d.invalid = true;
            return;
        }
        // conversion-type-id: type-specifier-seq conversion-declarator(opt).
        let ds = self.parse_decl_specifiers(DeclaratorContext::TypeName);
        if !ds.has_type_specifier() {
            self.error_here(DiagId::parse_expected_type);
            d.invalid = true;
            return;
        }
        let mut cd = Declarator::new(DeclaratorContext::TypeName, self.loc());
        self.parse_conversion_declarator(&mut cd);
        let base = self.s.type_from_declspec(&ds);
        let ty = self.s.type_for_declarator(base, &cd);
        if ds.invalid || cd.invalid {
            d.invalid = true;
        }
        d.special = Some((SpecialName::Conversion(ty), loc));
    }

    /// conversion-declarator ([class.conv.fct]/1): ptr-operators only,
    /// taken greedily.
    fn parse_conversion_declarator(&mut self, d: &mut Declarator) {
        let loc = self.loc();
        match Self::punct_of(self.tok()) {
            Some(Punct::Star) => {
                self.bump();
                let quals = self.parse_ptr_cv();
                self.parse_conversion_declarator(d);
                d.chunks.push(Chunk::Pointer { quals, loc });
            }
            Some(Punct::Amp) => {
                self.bump();
                self.parse_conversion_declarator(d);
                d.chunks.push(Chunk::LRef { loc });
            }
            Some(Punct::AmpAmp) => {
                self.bump();
                self.parse_conversion_declarator(d);
                d.chunks.push(Chunk::RRef { loc });
            }
            _ => {}
        }
    }

    /// The operator of an operator-function-id ([over.oper.general]/1),
    /// after `operator`; consumes it.
    fn parse_operator_symbol(&mut self) -> Option<OverloadedOperator> {
        use OverloadedOperator as O;
        if let Some(k @ (Kw::New | Kw::Delete)) = self.kw() {
            self.bump();
            let array = self.at(Punct::LSquare) && self.peek_is(1, Punct::RSquare);
            if array {
                self.bump();
                self.bump();
            }
            return Some(match (k, array) {
                (Kw::New, false) => O::New,
                (Kw::New, true) => O::ArrayNew,
                (_, false) => O::Delete,
                (_, true) => O::ArrayDelete,
            });
        }
        if self.at_kw(Kw::CoAwait) {
            self.bump();
            return Some(O::Coawait);
        }
        let p = Self::punct_of(self.tok())?;
        let op = match p {
            Punct::LParen if self.peek_is(1, Punct::RParen) => {
                self.bump();
                O::Call
            }
            Punct::LSquare if self.peek_is(1, Punct::RSquare) => {
                self.bump();
                O::Subscript
            }
            Punct::Plus => O::Plus,
            Punct::Minus => O::Minus,
            Punct::Star => O::Star,
            Punct::Slash => O::Slash,
            Punct::Percent => O::Percent,
            Punct::Caret => O::Caret,
            Punct::Amp => O::Amp,
            Punct::Pipe => O::Pipe,
            Punct::Tilde => O::Tilde,
            Punct::Exclaim => O::Exclaim,
            Punct::Equal => O::Equal,
            Punct::Less => O::Less,
            Punct::Greater => O::Greater,
            Punct::PlusEqual => O::PlusEqual,
            Punct::MinusEqual => O::MinusEqual,
            Punct::StarEqual => O::StarEqual,
            Punct::SlashEqual => O::SlashEqual,
            Punct::PercentEqual => O::PercentEqual,
            Punct::CaretEqual => O::CaretEqual,
            Punct::AmpEqual => O::AmpEqual,
            Punct::PipeEqual => O::PipeEqual,
            Punct::LessLess => O::LessLess,
            Punct::GreaterGreater => O::GreaterGreater,
            Punct::LessLessEqual => O::LessLessEqual,
            Punct::GreaterGreaterEqual => O::GreaterGreaterEqual,
            Punct::EqualEqual => O::EqualEqual,
            Punct::ExclaimEqual => O::ExclaimEqual,
            Punct::LessEqual => O::LessEqual,
            Punct::GreaterEqual => O::GreaterEqual,
            Punct::Spaceship => O::Spaceship,
            Punct::AmpAmp => O::AmpAmp,
            Punct::PipePipe => O::PipePipe,
            Punct::PlusPlus => O::PlusPlus,
            Punct::MinusMinus => O::MinusMinus,
            Punct::Comma => O::Comma,
            Punct::ArrowStar => O::ArrowStar,
            Punct::Arrow => O::Arrow,
            _ => return None,
        };
        self.bump();
        Some(op)
    }

    // -- base clauses -------------------------------------------------------------------

    /// base-clause ([class.derived.general]), at `:`.
    fn parse_base_clause(&mut self, bases: &mut Vec<BaseSpecInfo>) {
        self.bump();
        loop {
            match self.parse_base_specifier() {
                Some(b) => bases.push(b),
                None => {
                    self.skip_until(&[Punct::Comma, Punct::LBrace], true, false);
                }
            }
            if self.eat(Punct::Comma).is_none() {
                break;
            }
        }
    }

    /// base-specifier, with a trailing `...` (clang's `ParseBaseSpecifier`).
    fn parse_base_specifier(&mut self) -> Option<BaseSpecInfo> {
        let loc = self.loc();
        let mut attrs = Vec::new();
        if self.at_cxx11_attr() {
            self.parse_attributes(&mut attrs);
        }
        let mut is_virtual = self.eat_kw(Kw::Virtual).is_some();
        let access = match self.kw() {
            Some(Kw::Public) => Access::Public,
            Some(Kw::Protected) => Access::Protected,
            Some(Kw::Private) => Access::Private,
            _ => Access::None,
        };
        if access != Access::None {
            self.bump();
        }
        if let Some(l) = self.eat_kw(Kw::Virtual) {
            if is_virtual {
                self.s.error(DiagId::parse_dup_virtual_base, l);
            }
            is_virtual = true;
        }
        // class-or-decltype ([class.derived.general]/1): a decltype-specifier
        // (WP-074). `decltype(auto)` is not a class-or-decltype.
        if self.at_kw(Kw::Decltype) {
            let at = self.loc();
            let (kind, ty) = self.parse_decltype_specifier();
            if matches!(kind, TypeSpecKind::DecltypeAuto) {
                self.s.error(DiagId::parse_decltype_auto_invalid, at);
                return None;
            }
            let ty = ty?;
            let pack_expansion = self.eat(Punct::Ellipsis).is_some();
            return Some(BaseSpecInfo { ty, loc, is_virtual, access, pack_expansion, attrs });
        }
        // A simple-template-id or dependent class-name ([class.derived.general]/1,
        // [temp.res.general]/4: implicitly a type here).
        if self.at_template_type_name(DeclaratorContext::TypeName) {
            let ty = self.parse_template_type_name(DeclaratorContext::TypeName)?;
            let pack_expansion = self.eat(Punct::Ellipsis).is_some();
            return Some(BaseSpecInfo { ty, loc, is_virtual, access, pack_expansion, attrs });
        }
        let Ok(q) = self.parse_nested_name() else {
            return None;
        };
        let Some(name) = self.ident() else {
            self.error_here(DiagId::parse_expected_class_name);
            return None;
        };
        let id_loc = self.bump().span.lo;
        let NameClass::Type(ty, d) = self.s.classify_name(name, q) else {
            self.s.error(DiagId::parse_expected_class_name, id_loc);
            return None;
        };
        let ty = self.named_type(ty, d);
        let pack_expansion = self.eat(Punct::Ellipsis).is_some();
        Some(BaseSpecInfo { ty, loc, is_virtual, access, pack_expansion, attrs })
    }

    // -- after a class or enum definition ---------------------------------------------

    /// After a class or enum definition used as a type-specifier: if the
    /// next token cannot follow one, reports the missing `;` and inserts
    /// it (clang's `isValidAfterTypeSpecifier` recovery). `spelling` is
    /// the class-key or `enum`.
    pub(crate) fn check_after_tag_definition(&mut self, spelling: &str, could_be_bitfield: bool) {
        if self.is_valid_after_tag(could_be_bitfield) {
            return;
        }
        if self.at(Punct::Colon) {
            let d = self.s.diag(DiagId::parse_expected_after, self.loc()).arg(spelling).arg("';'");
            self.emit(d);
            self.bump();
        } else {
            let d =
                self.s.diag(DiagId::parse_expected_after, self.prev_end()).arg(spelling).arg("';'");
            self.emit(d);
        }
        self.inject_semi();
    }

    fn is_valid_after_tag(&self, could_be_bitfield: bool) -> bool {
        let t = self.tok();
        if let Some(p) = Self::punct_of(t) {
            return match p {
                Punct::Semi
                | Punct::Star
                | Punct::Amp
                | Punct::AmpAmp
                | Punct::RParen
                | Punct::ColonColon
                | Punct::LParen
                | Punct::Comma
                | Punct::LSquare
                | Punct::Ellipsis
                | Punct::Greater => true,
                Punct::Colon => could_be_bitfield,
                _ => false,
            };
        }
        match self.kw_of(t) {
            None => self.ident_of(t).is_some(),
            Some(Kw::Decltype | Kw::Operator | Kw::Declspec | Kw::Attribute) => true,
            Some(
                Kw::Const
                | Kw::Volatile
                | Kw::Restrict
                | Kw::Inline
                | Kw::Virtual
                | Kw::Friend
                | Kw::Static
                | Kw::Extern
                | Kw::Typedef
                | Kw::Register
                | Kw::Auto
                | Kw::Mutable
                | Kw::ThreadLocal
                | Kw::Constexpr
                | Kw::Consteval
                | Kw::Constinit,
            ) => !self.kw_of(self.peek(1)).is_some_and(|k| {
                matches!(
                    k,
                    Kw::Short
                        | Kw::Long
                        | Kw::Signed
                        | Kw::Unsigned
                        | Kw::Void
                        | Kw::Char
                        | Kw::WChar
                        | Kw::Char8
                        | Kw::Char16
                        | Kw::Char32
                        | Kw::Int
                        | Kw::Int128
                        | Kw::Float
                        | Kw::Double
                        | Kw::Float128
                        | Kw::Bool
                        | Kw::Class
                        | Kw::Struct
                        | Kw::Union
                        | Kw::Enum
                        | Kw::Decltype
                        | Kw::Typename
                )
            }),
            Some(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    //! `act_on_*` sequence tests (spec/parse.md §9) and the token capture
    //! and replay machinery (§2.4), driven through a real [`Parser`].

    use std::sync::Arc;

    use kiln_ast::Ast;
    use kiln_ast::types::TypeContext;
    use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
    use kiln_lex::literal::LiteralOptions;
    use kiln_lex::{Punct, Std};
    use kiln_pp::{PpOptions, Preprocessor};
    use kiln_sema::Sema;
    use kiln_sema::target::{Arch, Target};
    use kiln_source::SourceManager;
    use kiln_source::symbol::Interner;

    use crate::parser::Parser;
    use crate::tokens::TokenSource;

    /// Runs `f` on a parser positioned at the first token of `src`, and
    /// returns its result and the diagnostics' messages.
    fn with_parser<R>(src: &str, f: impl FnOnce(&mut Parser<'_, '_>) -> R) -> (R, Vec<String>) {
        let sm = SourceManager::new();
        let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
        let interner = Arc::new(Interner::new());
        let sink = MemorySink::new();
        let diags = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
        let target = Target::for_arch(Arch::X86_64);
        let mut pp = Preprocessor::new(&sm, &diags, &interner, PpOptions::new(Std::Cxx26));
        pp.enter_main_file(file);
        let lit = LiteralOptions::new(Std::Cxx26, crate::encoding_params(&target));
        let ast = Ast::new(Arc::clone(&interner), TypeContext::new(target.types.clone()));
        let s = Sema::for_target(ast, &diags, target);
        let mut p = Parser::new(s, TokenSource::new(pp, &diags), lit);
        let r = f(&mut p);
        drop(p);
        (r, sink.take().into_iter().map(|d| d.message).collect())
    }

    /// The trace of parsing `src` as a translation unit.
    fn trace(src: &str) -> Vec<String> {
        with_parser(src, |p| {
            p.parse_translation_unit();
            std::mem::take(&mut p.trace)
        })
        .0
    }

    fn trace_is(src: &str, want: &[&str]) {
        assert_eq!(trace(src), want, "trace of {src:?}");
    }

    /// [class.pre]: the sequence for a plain class definition.
    #[test]
    fn seq_simple_class() {
        trace_is(
            "struct S { int a; };",
            &[
                "start_record_definition(S)",
                "base_specifiers(0)",
                "declarator(a)",
                "finish_record_definition(S, semi)",
                "finish_class_contexts(S)",
            ],
        );
    }

    /// [class.pre]/3: `final` is reported before the base clause, and a
    /// declarator after `}` means no `;` directly follows it.
    #[test]
    fn seq_final_and_declarator() {
        trace_is(
            "struct S final { int a; } s;",
            &[
                "start_record_definition(S)",
                "class_final",
                "base_specifiers(0)",
                "declarator(a)",
                "finish_record_definition(S, nosemi)",
                "finish_class_contexts(S)",
            ],
        );
    }

    /// [class.derived], [class.access.spec]: bases and access labels.
    #[test]
    fn seq_bases_and_access() {
        let t =
            trace("struct B {}; struct C {}; struct S : B, virtual public C { public: int a; };");
        let s = t.iter().position(|x| x == "start_record_definition(S)").expect("S");
        assert_eq!(
            t[s..],
            [
                "start_record_definition(S)",
                "base_specifiers(2)",
                "access_spec",
                "declarator(a)",
                "finish_record_definition(S, semi)",
                "finish_class_contexts(S)",
            ]
        );
    }

    /// [class.friend]: friend type and friend function declarations.
    #[test]
    fn seq_friends() {
        trace_is(
            "struct S { friend class F; friend int g(); };",
            &[
                "start_record_definition(S)",
                "base_specifiers(0)",
                "friend",
                "friend",
                "finish_record_definition(S, semi)",
                "finish_class_contexts(S)",
            ],
        );
    }

    /// [class.mem.general]/7: default member initializers are parsed after
    /// the outermost class is complete, nested classes first, each class's
    /// in a `start/finish_delayed_class` pair; only the outermost class
    /// finishes its complete-class contexts.
    #[test]
    fn seq_nested_nsdmi_phase() {
        trace_is(
            "struct O { struct I { int x = 1; }; int y = 2; };",
            &[
                "start_record_definition(O)",
                "base_specifiers(0)",
                "start_record_definition(I)",
                "base_specifiers(0)",
                "declarator(x)",
                "finish_record_definition(I, semi)",
                "decl_spec_only",
                "declarator(y)",
                "finish_record_definition(O, semi)",
                "start_delayed_class(O)",
                "start_delayed_class(I)",
                "start_field_init(x)",
                "field_init(x)",
                "finish_delayed_class(I)",
                "start_field_init(y)",
                "field_init(y)",
                "finish_delayed_class(O)",
                "finish_class_contexts(O)",
            ],
        );
    }

    /// [class.mem.general]/7: a default member initializer sees members
    /// declared after it (it is replayed once the class is complete).
    #[test]
    fn nsdmi_sees_later_members() {
        let ((), diags) = with_parser("struct S { int x = y; static const int y = 1; };", |p| {
            p.parse_translation_unit();
        });
        assert!(!diags.iter().any(|d| d.contains("undeclared")), "{diags:?}");
    }

    /// [dcl.fct.def.general]: a namespace-scope definition is bracketed by
    /// `start/finish_function_def`; `= delete` is reported as such.
    #[test]
    fn seq_function_definitions() {
        trace_is("void f() {}", &["start_function_def(f)", "finish_function_def(f)"]);
        trace_is("void f() = delete;", &["deleted"]);
    }

    /// [class.pre]: a missing `;` after `}` finishes the class with
    /// `nosemi` and parsing continues with the next declaration.
    #[test]
    fn seq_missing_semi_recovers() {
        let t = trace("struct S { int a; } struct T {};");
        assert!(t.contains(&"finish_record_definition(S, nosemi)".to_owned()), "{t:?}");
        assert!(t.contains(&"finish_class_contexts(T)".to_owned()), "{t:?}");
    }

    /// [class.mem.general]: an unterminated class still finishes its
    /// definition and contexts (no panic, no unbalanced replay).
    #[test]
    fn seq_unterminated_class() {
        let t = trace("struct S { int a = 1; int f(int = 2) {}");
        assert_eq!(t.first().map(String::as_str), Some("start_record_definition(S)"));
        assert_eq!(t.last().map(String::as_str), Some("finish_class_contexts(S)"));
    }

    /// spec/parse.md §2.4: a saved default argument stops at the `,` that
    /// ends it (balancing brackets), and replay restores the stream.
    #[test]
    fn capture_and_replay_default_arg() {
        let ((n, after, all, back), _) = with_parser("= g(1, 2) + a[3] , next", |p| {
            let late = p.capture_default_arg();
            let after = p.at(Punct::Comma);
            let n = late.toks.len();
            p.begin_replay(late.toks, late.end);
            while !p.at_eof() {
                p.bump();
            }
            let all = p.end_replay();
            (n, after, all, p.at(Punct::Comma))
        });
        // `= g ( 1 , 2 ) + a [ 3 ]`
        assert_eq!(n, 12);
        assert!(after && all && back);
    }

    /// spec/parse.md §2.4: a default argument ends at the `)` of its
    /// parameter list, and a `?:` keeps its `:` inside.
    #[test]
    fn capture_default_arg_conditional() {
        let ((n, closed), _) = with_parser("= b ? 1 : 2 )", |p| {
            let late = p.capture_default_arg();
            (late.toks.len(), p.at(Punct::RParen))
        });
        assert_eq!(n, 6);
        assert!(closed);
    }

    /// spec/parse.md §2.4: replay that does not consume every saved token
    /// reports it, and still restores the interrupted stream.
    #[test]
    fn replay_partial_consumption() {
        let ((all, back), _) = with_parser("= 1 2 ) tail", |p| {
            let late = p.capture_default_arg();
            p.begin_replay(late.toks, late.end);
            p.bump();
            let all = p.end_replay();
            (all, p.at(Punct::RParen))
        });
        assert!(!all);
        assert!(back);
    }
}
