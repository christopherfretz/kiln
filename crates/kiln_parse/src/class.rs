//! Classes ([class], [class.mem], [class.derived], [class.base.init];
//! spec/parse.md §2.7, §4.5): class-specifier bodies, base clauses,
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
use kiln_ast::types::TagKeyword;
use kiln_ast::{DeclId, QualType};
use kiln_diag::DiagId;
use kiln_lex::{Punct, TokKind, Token};
use kiln_sema::{
    BaseSpecInfo, DeclSpec, Declarator, DeclaratorContext, Initializer, MemInitId, MemInitInfo,
    NameClass, SpecialName, StorageSpec,
};
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

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

    /// Saves a delayed default argument at `=` (spec/parse.md §2.7; clang's
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
                        || matches!(
                            Self::punct_of(t),
                            Some(Punct::Greater | Punct::GreaterGreater)
                        )
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
    fn skip_deleted_function_body(&mut self) {
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
                    let close =
                        if self.at(Punct::LParen) { Punct::RParen } else { Punct::RSquare };
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
    /// in declaration order, each class's inside its re-entered scope.
    fn replay_phase(&mut self, class: &mut LateClass, ph: Phase) {
        if !class.has(ph) {
            return;
        }
        let r = class.record;
        self.trace(|p| format!("start_delayed_class({})", p.decl_name(r)));
        self.s.act_on_start_delayed_class(r);
        for item in &mut class.items {
            match (item, ph) {
                (LateItem::Method { f, parts }, Phase::Method) => {
                    let f = *f;
                    let parts = mem::take(parts);
                    self.replay_method(f, parts);
                }
                (LateItem::Nsdmi { field, toks }, Phase::Nsdmi) => {
                    let field = *field;
                    let toks = mem::take(&mut toks.toks);
                    let end = match item_end(class_end_dummy()) {
                        _ => Loc::INVALID,
                    };
                    let _ = (field, toks, end);
                }
                _ => {}
            }
        }
        self.trace(|p| format!("finish_delayed_class({})", p.decl_name(r)));
        self.s.act_on_finish_delayed_class(r);
    }
}
