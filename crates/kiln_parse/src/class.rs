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
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::types::TagKeyword;
use kiln_ast::{DeclId, QualType};
use kiln_diag::DiagId;
use kiln_lex::{Punct, TokFlags, TokKind, Token};
use kiln_sema::{
    BaseSpecInfo, Chunk, DeclSpec, Declarator, DeclaratorContext, Initializer, MemInitId,
    MemInitInfo, NameClass, SpecialName, StorageSpec, TypeSpecKind,
};
use kiln_sema::{ContractDeclKind, DeclSite};
use kiln_source::symbol::Symbol;
use kiln_source::{Loc, Span};

use crate::decl::{is_deduction_guide, is_untyped_special};
use crate::fp_pragma::FpAt;
use crate::keywords::Kw;
use crate::parser::{Parser, quote};
use kiln_sema::templ::auto::AutoGroup;

/// Saved tokens and the location of the `Eof` sentinel that ends their
/// replay.
pub(crate) struct LateToks {
    pub(crate) toks: Vec<Token>,
    pub(crate) end: Loc,
}

impl LateToks {
    /// Where the saved tokens after the first start (a default
    /// argument's expression after its `=`), else `or`.
    pub(crate) fn second_loc(&self, or: Loc) -> Loc {
        self.toks.get(1).map_or(or, |t| t.span.lo)
    }
}

/// The saved parts of a member function declarator: its default
/// arguments (by parameter index) and its *noexcept-specifier*. The
/// `delay_*` switches tell the declarator parser whether to save them.
#[derive(Default)]
pub(crate) struct LateFnParts {
    pub(crate) default_args: Vec<(usize, LateToks)>,
    pub(crate) noexcept: Option<LateToks>,
    /// The *function-contract-specifier-seq* and what the declaration
    /// is ([dcl.contract.func]/6), replayed with the noexcept-specifier.
    pub(crate) contracts: Option<(LateToks, ContractDeclKind)>,
    pub(crate) delay_args: bool,
    pub(crate) delay_noexcept: bool,
}

impl LateFnParts {
    fn is_empty(&self) -> bool {
        self.default_args.is_empty() && self.noexcept.is_none() && self.contracts.is_none()
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
    /// The *function-contract-specifier-seq* of an inline definition `f`
    /// with a deduced return type, replayed after its body
    /// ([dcl.contract.res]/2).
    Contracts { f: DeclId, toks: LateToks },
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
            LateItem::Body { .. } | LateItem::Contracts { .. } => ph == Phase::Body,
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
    pub(crate) fn take_into(&mut self, out: &mut Vec<Token>) {
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

    /// clang's `ConsumeAndStoreInitializer`: moves a default argument or
    /// default member initializer into `out`, up to (not including) the
    /// `,`, `)` or `;` that ends it. A `,` after an unclosed `<` may be a
    /// template argument separator: a tentative parse of what follows
    /// decides ([`Parser::comma_ends_saved_initializer`]); once one such
    /// comma did not end it, the `<` is known to open a template argument
    /// list. Returns false at end of input or at an unbalanced closer.
    pub(crate) fn capture_initializer(&mut self, kind: InitKind, out: &mut Vec<Token>) -> bool {
        let mut first = true;
        // clang's `AngleCount` (possible unclosed `<`s) and
        // `KnownTemplateCount`.
        let (mut angles, mut known) = (0u32, 0u32);
        loop {
            let t = self.tok();
            match Self::punct_of(t) {
                Some(Punct::Comma) => {
                    if angles == 0 {
                        return true;
                    }
                    if known == 0 {
                        if self.comma_ends_saved_initializer(kind == InitKind::Nsdmi) {
                            return true;
                        }
                        known += 1;
                    }
                    self.take_into(out);
                }
                Some(Punct::Less) => {
                    angles += 1;
                    self.take_into(out);
                }
                Some(p @ (Punct::Greater | Punct::GreaterGreater)) => {
                    let n = if p == Punct::Greater { 1 } else { 2 };
                    angles = angles.saturating_sub(n);
                    known = known.saturating_sub(n);
                    self.take_into(out);
                }
                Some(Punct::Question) => {
                    if !self.capture_conditional(out) {
                        return false;
                    }
                }
                Some(open @ (Punct::LParen | Punct::LSquare | Punct::LBrace)) => {
                    self.take_into(out);
                    // Nested brackets may hold a lambda body's `;`.
                    if let Some(close) = closer_of(open) {
                        self.capture_until(&[close], out, false, true);
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
                _ if self.kw_of(t) == Some(Kw::Template) => {
                    // `template name <` certainly opens template arguments.
                    self.take_into(out);
                    if self.ident().is_some() {
                        self.take_into(out);
                        if self.at(Punct::Less) {
                            angles += 1;
                            known += 1;
                            self.take_into(out);
                        }
                    }
                }
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
    pub(crate) fn expected_here(&self, what: &str) {
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

    /// "expected member name" at the current token, highlighting the
    /// decl-specifier-seq `ds_range` (clang's `DS.getSourceRange()`, from
    /// a leading attribute-specifier-seq).
    fn error_expected_member_name(&mut self, ds_range: Span) {
        let e = self.s.diag(DiagId::parse_expected_member_name, self.loc()).range(ds_range);
        self.emit(e);
    }

    /// clang's `SkipMalformedDecl`: skips to the end of a broken member
    /// declaration (through a `;` or a function body, or up to a `}`).
    pub(crate) fn skip_malformed_decl(&mut self) {
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
    /// class, then replays the saved complete-class contexts. `in_type_id`
    /// says the class is defined in a type-id (`using A = struct {...};`),
    /// where a following `;` does not make it an anonymous struct or
    /// union ([class.union.anon]/1 needs a declaration with no
    /// declarators).
    pub(crate) fn parse_record_body(
        &mut self,
        r: DeclId,
        final_loc: Option<Loc>,
        in_type_id: bool,
    ) {
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
            self.take_fp_pragmas(FpAt::Class(self.class_key_name()));
            self.expect_closing(Punct::RBrace, Punct::LBrace, open);
            // clang `ParseCXXMemberSpecification`: GNU attributes after
            // the `}` appertain to the class.
            if self.at_kw(Kw::Attribute) {
                let mut attrs = Vec::new();
                self.parse_gnu_attr_seq(&mut attrs);
                self.trace(|_| format!("record_trailing_attrs({})", attrs.len()));
                self.s.act_on_record_trailing_attrs(r, &attrs);
            }
        } else {
            self.s.error(DiagId::parse_expected_lbrace_after_base, self.prev_end());
        }
        let then_semi = !in_type_id && self.at(Punct::Semi);
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
        // A member class template of a non-template class is replayed
        // after its head closed: its parameters are visible again.
        let heads = self.s.act_on_reenter_template_scopes(r);
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
                LateItem::Contracts { f, toks } if ph == Phase::Body => {
                    let toks = LateToks { toks: mem::take(&mut toks.toks), end: toks.end };
                    let n = self.s.act_on_reenter_template_scopes(*f);
                    self.s.act_on_start_delayed_method(*f);
                    self.replay_member_contract_specs(*f, toks, ContractDeclKind::Definition);
                    self.s.act_on_finish_delayed_method(*f);
                    self.s.act_on_exit_reentered_template_scopes(n);
                }
                LateItem::Nested(c) => self.replay_phase(c, ph),
                _ => {}
            }
        }
        self.trace(|p| format!("finish_delayed_class({})", p.decl_name(r)));
        self.s.act_on_finish_delayed_class(r);
        self.s.act_on_exit_reentered_template_scopes(heads);
    }

    /// A friend function redeclaration `f` without a seq is still
    /// checked against the seq of its first declaration: the const rule
    /// on postcondition parameters holds on every declaration
    /// ([dcl.contract.func]/7).
    fn check_friend_without_contracts(
        &mut self,
        is_fn: bool,
        ds: &DeclSpec,
        parts: &LateFnParts,
        f: Option<DeclId>,
        d: &Declarator,
    ) {
        if is_fn && ds.friend.is_some() && parts.contracts.is_none() && f.is_some() {
            self.s.act_on_function_contracts(f, &[], ContractDeclKind::Declaration, d.loc);
        }
    }

    /// A member function declarator's *function-contract-specifier-seq*,
    /// captured with what the declaration is ([dcl.contract.func]/6).
    fn capture_member_contract_specs(&mut self) -> Option<(LateToks, ContractDeclKind)> {
        let t = self.capture_contract_specs()?;
        let how = if self.at(Punct::Equal) {
            match self.kw_of(self.peek(1)) {
                Some(Kw::Delete) => ContractDeclKind::Deleted,
                Some(Kw::Default) => ContractDeclKind::Defaulted,
                _ => ContractDeclKind::Declaration,
            }
        } else if self.at_function_definition() {
            ContractDeclKind::Definition
        } else {
            ContractDeclKind::Declaration
        };
        Some((t, how))
    }

    /// Skips what is left of the saved tokens and ends the replay.
    pub(crate) fn finish_replay(&mut self) {
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
            let eq = self.bump().span.lo;
            if let Some(&p) = params.get(i) {
                self.s.act_on_start_delayed_default_arg(p);
            }
            let mark = self.s.begin_default_arg_uses();
            if self.at(Punct::LBrace) {
                // `= { ... }` ([dcl.fct.default]/1: an initializer-clause).
                if let Ok(l) = self.parse_init_list() {
                    if !self.at_eof() {
                        self.error_here(DiagId::parse_default_arg_unparsed);
                    }
                    if let Some(&p) = params.get(i) {
                        self.trace(|_| format!("default_arg({i})"));
                        let pty = self.s.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                        if !self.s.reject_param_pack_default(pty, eq) {
                            self.s.act_on_param_default_list(p, &l);
                        }
                    }
                }
            } else if let Some(e) = self.parse_assignment_expr() {
                if !self.at_eof() {
                    self.error_here(DiagId::parse_default_arg_unparsed);
                }
                if let Some(&p) = params.get(i) {
                    self.trace(|_| format!("default_arg({i})"));
                    // [dcl.fct.default]/9 (checked on the written
                    // argument, as for one parsed in place).
                    if self.s.check_default_arg_refs(e) {
                        self.s.act_on_param_default_arg(p, e, eq);
                    } else {
                        self.s.ast.decls.set_flags(p, DeclFlags::INVALID);
                    }
                }
            }
            let arg = params
                .get(i)
                .filter(|&&p| !self.s.ast.decls.get(p).has(DeclFlags::INVALID))
                .and_then(|&p| self.s.ast.decls.param(p))
                .and_then(|x| x.default_arg);
            self.s.end_default_arg_uses(mark, arg);
            self.s.act_on_end_delayed_default_arg();
            self.finish_replay();
        }
        if let Some(t) = parts.noexcept {
            self.begin_replay(t.toks, t.end);
            // [expr.prim.this]/2: `this` may appear in a member
            // function's noexcept-specifier.
            let this = self.s.act_on_start_delayed_exception_spec(f);
            let spec = self.parse_exception_spec();
            self.s.act_on_end_this_scope(this);
            if !self.at_eof() {
                self.error_here(DiagId::parse_except_spec_unparsed);
            }
            self.trace(|_| "exception_spec".to_owned());
            self.s.act_on_exception_spec(f, spec);
            self.finish_replay();
        }
        if let Some((t, how)) = parts.contracts {
            self.replay_member_contract_specs(f, t, how);
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
        // clang 19's `ActOnFinishCXXInClassMemberInitializer` corrects the
        // delayed typos before the initialization, with no filter (WP-408).
        let typos = self.begin_typo_correction();
        let init = self.parse_member_initializer(false);
        self.correct_typos(typos, init.is_some());
        if let Some(init) = init {
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
        // function-try-block ([except.pre]/1): `try ctor-initializer?
        // compound-statement handler-seq` (clang `ParseFunctionTryBlock`).
        let fn_try = self.eat_kw(Kw::Try);
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
        if let Some(try_loc) = fn_try {
            let body = self.parse_try_block_common(try_loc);
            let (body, end) = match body {
                Some(t) => {
                    // The `}` of the last handler ends the definition.
                    let st = &self.s.ast.stmts;
                    let last = self.s.ast.lists.iter::<kiln_ast::StmtId>(st.try_handlers(t)).last();
                    let end = last.map_or(try_loc, |h| st.rbrace(st.catch_block(h)));
                    (t, end)
                }
                None => (self.s.act_on_compound(&[], try_loc, try_loc), try_loc),
            };
            self.trace(|p| format!("finish_function_def({})", p.decl_name(f)));
            self.sync_attr_macros();
            self.s.act_on_finish_function_def(f, body, end);
            return;
        }
        let (body, rbrace) = self.parse_compound(false);
        self.trace(|p| format!("finish_function_def({})", p.decl_name(f)));
        self.sync_attr_macros();
        self.s.act_on_finish_function_def(f, body, rbrace);
    }

    /// mem-initializer ([class.base.init]): the initializer (when sema can
    /// take it) and whether it parsed.
    fn parse_mem_initializer(&mut self) -> (Option<MemInitInfo>, bool) {
        // A simple-template-id class-name, possibly qualified (`Leaf<I, T>(u)...`,
        // [class.base.init]/2), names the base as a base-specifier would.
        if self.at_template_type_name(DeclaratorContext::TypeName) {
            let loc = self.loc();
            let Some(ty) = self.parse_template_type_name(DeclaratorContext::TypeName) else {
                return (None, false);
            };
            let Some(init) = self.parse_mem_init_args() else {
                return (None, false);
            };
            let pack_expansion = self.eat(Punct::Ellipsis).is_some();
            return (
                Some(MemInitInfo { id: MemInitId::Type(ty), loc, init, pack_expansion }),
                true,
            );
        }
        let Ok(qual) = self.parse_nested_name_nns() else {
            return (None, false);
        };
        let q = qual.map(|q| q.ctx);
        let loc = self.loc();
        // class-or-decltype ([class.base.init]/1): `decltype(e)` names
        // the base by type (a dependent one is resolved at instantiation).
        if qual.is_none() && self.at_kw(Kw::Decltype) {
            let (kind, ty) = self.parse_decltype_specifier();
            let Some(init) = self.parse_mem_init_args() else {
                return (None, false);
            };
            let pack_expansion = self.eat(Punct::Ellipsis).is_some();
            if kind == TypeSpecKind::DecltypeAuto {
                self.s.error(DiagId::parse_decltype_auto_invalid, loc);
            }
            // After an error the initializer is still counted, as failed
            // (no implicit base or member initializers are added).
            let ty = ty.filter(|_| kind != TypeSpecKind::DecltypeAuto).unwrap_or(QualType::ERROR);
            return (
                Some(MemInitInfo { id: MemInitId::Type(ty), loc, init, pack_expansion }),
                true,
            );
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
        // [class.access]/4: a mem-initializer-id naming a type (a base,
        // through a typedef or its injected-class-name) is access-checked.
        if let NameClass::Type(_, d) = self.s.classify_name(name, q) {
            self.s.check_type_name_access(name, q, d, loc);
        }
        let id = match q {
            // `Bs(args)...` names a pack: resolve the type now so that
            // instantiation can expand it ([temp.variadic]/5).
            None if pack_expansion => match self.s.classify_name(name, None) {
                NameClass::Type(ty, d) => MemInitId::Type(self.named_type(ty, d)),
                _ => MemInitId::Ident(name),
            },
            // A template type parameter cannot be redeclared by a member
            // ([temp.local]/6), so naming one is a base: resolve it now,
            // since instantiation re-resolves identifiers in the
            // instantiated class, where the parameter is not in scope.
            None => match self.s.classify_name(name, None) {
                NameClass::Type(ty, d)
                    if self.s.ast.decls.get(d).kind() == DeclKind::TemplateTypeParm =>
                {
                    MemInitId::Type(self.named_type(ty, d))
                }
                _ => MemInitId::Ident(name),
            },
            Some(q) => {
                if let NameClass::Type(ty, d) = self.s.classify_name(name, Some(q)) {
                    MemInitId::Type(self.named_type(ty, d))
                } else if let Some(d) = self.s.inherited_injected_class(name, Some(q)) {
                    // `D::B()` naming the injected-class-name of a base
                    // class template specialization ([temp.local]/1-2,
                    // [class.base.init]/2).
                    self.s.mark_type_referenced(d);
                    MemInitId::Type(self.s.elaborated_tag_type(d, TagKeyword::None))
                } else if let Some(ty) = qual.and_then(|qual| {
                    self.s.act_unknown_specialization_type(qual, name, TagKeyword::None, loc)
                }) {
                    // `T::B(1)`, or `D::B(1)` reaching a dependent base:
                    // a type resolved at instantiation ([class.base.init]/2).
                    MemInitId::Type(ty)
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
    /// The class-key of the class being defined, as clang's
    /// `err_pragma_misplaced_in_decl` names it.
    fn class_key_name(&self) -> &'static str {
        let tag = self.late.last().and_then(|c| self.s.ast.decls.record(c.record));
        match tag.map(|r| r.tag) {
            Some(kiln_ast::decl::TagKind::Class) => "class",
            Some(kiln_ast::decl::TagKind::Union) => "union",
            _ => "struct",
        }
    }

    fn parse_member_declaration_leader(&mut self) -> bool {
        self.skip_extension_before_using();
        let loc = self.loc();
        // A loop-hint or FP pragma in a class (clang
        // `ParseCXXClassMemberDeclarationWithPragmas`): only the pragma is
        // skipped.
        if let Some(h) = self.take_loop_hints() {
            let tag = self.class_key_name();
            let d = self.s.diag(DiagId::parse_pragma_misplaced_in_decl, h[0].start).arg(tag);
            self.emit(d);
            return true;
        }
        if self.take_fp_pragmas(FpAt::Class(self.class_key_name())) {
            return true;
        }
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
            _ if self.at_consteval_block() => {
                self.parse_consteval_block();
                true
            }
            (_, Some(Kw::Using)) => {
                self.parse_using();
                true
            }
            _ if self.at_attributed_using() => {
                self.parse_attributed_using();
                true
            }
            (_, Some(Kw::Template)) if !self.peek_is(1, Punct::SpliceOpen) => {
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
            Ok(l) => {
                if l.is_some() {
                    d.end = self.prev_lo();
                }
                d.asm_label = l;
            }
            Err(()) => {
                self.skip_until(&[Punct::Comma], true, false);
            }
        }
        self.gnu_attr_end = Loc::INVALID;
        if d.is_function_declarator()
            && d.asm_label.is_none()
            && d.virt_final.is_none()
            && d.virt_override.is_none()
        {
            // `void f() [[a]];`: the function type's (not yet checked),
            // parsed with the function declarator in clang
            // (`ParseFunctionDeclarator`), so only a C++11 specifier-seq
            // right after it; one after the GNU attributes is reported.
            self.parse_cxx11_attr_seq(&mut d.attrs);
        }
        self.parse_member_tail_attrs(d);
        // GNU attributes (closed by `))`) extend the declarator's range;
        // C++11 ones (`]]`) do not (clang `MaybeParseGNUAttributes(D)`).
        if self.gnu_attr_end != Loc::INVALID {
            d.end = self.gnu_attr_end;
        }
        if !has_width && d.virt_final.is_none() && d.virt_override.is_none() {
            self.parse_virt_specifiers(d, friend);
        }
        // So does the last virt-specifier, wherever it was written.
        if let Some(v) = d.virt_final.max(d.virt_override) {
            d.end = v;
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
    /// clang forms each member declarator's type anew: a decl-specifier
    /// error is repeated per declarator ([`Parser::repeat_declspec_errors`])
    /// and every later declarator without a type is reported
    /// (`static y, z;`).
    fn repeat_member_declspec_errors(
        &mut self,
        ds: &DeclSpec,
        d: &Declarator,
        first: bool,
        restrict_error: &mut Option<(Loc, QualType)>,
    ) {
        self.repeat_declspec_errors(restrict_error, first);
        let untyped = !ds.has_type_specifier() && !ds.invalid;
        if !first && untyped && !d.invalid && d.special.is_none() {
            self.s.error(DiagId::parse_missing_type_specifier, d.name_loc());
        }
    }

    fn member_base_type(&mut self, ds: &DeclSpec, d: &Declarator) -> QualType {
        if !ds.has_type_specifier() && is_deduction_guide(d) {
            self.s.plain_auto_type()
        } else if !ds.has_type_specifier() && is_untyped_special(d) {
            QualType::VOID
        } else if !ds.has_type_specifier()
            && (d.invalid || (!d.has_name() && !self.at(Punct::Colon)))
        {
            // A malformed special declarator-id (`operator;`) is reported;
            // clang adds nothing about the type. Nor does it for a
            // declarator with no name (`static "C" ...`): clang reports only
            // "expected member name" and never builds the type.
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

    /// `friend friend-type-specifier-list ;` with a `...` or `,` after its
    /// first friend-type-specifier ([class.friend]/3, P2893; clang's
    /// variadic friend path in `ParseCXXClassMemberDeclaration`): one
    /// friend declaration per friend-type-specifier, each optionally a
    /// pack expansion. `ds` holds the first specifier; the `friend` at
    /// `fl` applies to all of them.
    fn parse_friend_type_list(&mut self, mut ds: DeclSpec, fl: Loc) {
        if self.std < kiln_lex::Std::Cxx26 {
            let d = self.s.diag(DiagId::parse_ext_variadic_friend, self.loc());
            self.emit(d);
        }
        loop {
            if !ds.has_type_specifier() {
                // `friend A, f;`: not a type (clang: the free-standing
                // decl-specifier-seq is invalid; the rest is skipped).
                self.error_here(DiagId::parse_missing_type_specifier);
                self.skip_to_semi();
                return;
            }
            let ellipsis = self.eat(Punct::Ellipsis);
            let base = self.s.type_from_declspec(&ds);
            let _ = self.s.act_on_friend_type(&ds, base, ellipsis, fl);
            if self.eat(Punct::Comma).is_none() {
                break;
            }
            ds = self.parse_decl_specifiers_from(DeclaratorContext::Member, Some(fl));
        }
        // clang's `ExpectAndConsume`: nothing is skipped.
        if self.eat(Punct::Semi).is_none() {
            let at = self.expected_loc();
            let d =
                self.s.diag(DiagId::parse_expected_semi_after_stmt, at).arg("friend declaration");
            self.emit(d);
        }
    }

    /// A member declaration with no decl-specifiers that is neither a
    /// special member nor a declarator without a type
    /// ([`Parser::at_implicit_int_declarator`]), skipped to its `;`
    /// ([dcl.spec.general]/3; also how a friend-type-specifier-list that
    /// lacks its `;` recovers, since nothing is skipped after it). `()` is
    /// clang's parenthesized declarator wanting a name, reported at the
    /// `)`. Anything else is "expected member name or ';'".
    /// A member declaration with no type-specifier whose declarator-id
    /// is a qualified name ([`Parser::at_qualified_implicit_int_declarator`]),
    /// except `R::g;`: an access declaration (not implemented), which
    /// keeps the "expected member name" recovery.
    fn at_qualified_member_declarator(&self) -> bool {
        let access = matches!(self.scan_name_qual_at(self.pos()),
            Some((end, false, _, _, Some(_))) if Self::punct_of(self.tok_at(end)) == Some(Punct::Semi));
        !access && self.at_qualified_implicit_int_declarator()
    }

    fn member_without_type(&mut self) {
        if self.at(Punct::LParen) && Self::punct_of(self.peek(1)) == Some(Punct::RParen) {
            self.bump();
        }
        self.error_here(DiagId::parse_expected_member_name);
        self.skip_to_semi();
    }

    /// True at a name that does not name a type followed by `;`, `=`,
    /// `,`, `[`, `(` or `{`, where no decl-specifier was written: a
    /// declarator without a type (`y;`, `y();`). clang declares it as an
    /// invalid `int` after "a type specifier is required for all
    /// declarations" at the name (`err_missing_type_specifier`), so later
    /// uses of the name are not diagnosed again.
    pub(crate) fn at_implicit_int_declarator(&self) -> bool {
        self.ident_of(self.tok()).is_some()
            && matches!(
                Self::punct_of(self.peek(1)),
                Some(
                    Punct::Semi
                        | Punct::Equal
                        | Punct::Comma
                        | Punct::LSquare
                        | Punct::LParen
                        | Punct::LBrace
                )
            )
    }

    /// The default member initializer of `decl`, saved for the complete
    /// class. [dcl.array]/3: for an array of unknown bound (`unbounded`)
    /// it is not an initializer that can deduce a bound (clang
    /// `err_incomplete_array_member_init`), at the initializer; the member
    /// is invalid (no flexible array member checks, spec/sema.md §5.8).
    fn save_member_nsdmi(&mut self, unbounded: bool, decl: Option<DeclId>) {
        if !unbounded {
            self.save_nsdmi(decl);
            return;
        }
        self.error_here(DiagId::parse_incomplete_array_member_init);
        self.skip_until(&[Punct::Comma], true, false);
        if let Some(x) = decl {
            self.s.ast.decls.set_flags(x, DeclFlags::INVALID);
        }
    }

    /// [dcl.attr.grammar]/5: an attribute-specifier-seq appertaining to
    /// a friend declaration requires it to be a definition: before a
    /// friend type or friend function declaration clang reports `an
    /// attribute list cannot appear here` at its start and drops it.
    fn prohibit_friend_lead_attrs(&mut self, ds: &mut DeclSpec) {
        if ds.friend.is_none() {
            return;
        }
        let Some(at) = ds.lead_cxx11 else { return };
        self.s.error(DiagId::parse_attributes_not_allowed, at);
        Self::drop_lead_cxx11(&self.s.ast.attrs, ds);
    }

    /// member-declaration ([class.mem]; clang's
    /// `ParseCXXClassMemberDeclaration`).
    pub(crate) fn parse_member_declaration(&mut self) {
        // Deprecation warnings in the decl-specifiers and first declarator
        // wait for the member they declare (clang's `ParsingDeclRAIIObject`).
        let level = self.s.begin_delayed_deprecation();
        let saved = self.member_depr.replace(level);
        self.parse_member_declaration_inner();
        self.member_depr = saved;
        self.s.end_delayed_deprecation_to(level, None);
    }

    /// After empty decl-specifiers: no declarator that may omit them
    /// (a special member, an implicit-int declarator, or a qualified
    /// member name) follows.
    fn at_member_without_type(&self) -> bool {
        !self.at_special_declarator(DeclaratorContext::Member)
            && !self.at_implicit_int_declarator()
            && !self.at_qualified_member_declarator()
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one member-declarator list, each kind handled inline"
    )]
    fn parse_member_declaration_inner(&mut self) {
        let tmpl_kind = self.template_decl_kind.take();
        if !self.stack_ok() || self.parse_member_declaration_leader() {
            return;
        }
        let mut ds = self.parse_decl_specifiers(DeclaratorContext::Member);
        if let Some(fl) = ds.friend
            && (self.at(Punct::Ellipsis) || self.at(Punct::Comma))
        {
            self.prohibit_friend_lead_attrs(&mut ds);
            self.parse_friend_type_list(ds, fl);
            return;
        }
        let mut tmpl_group = self.template_group(tmpl_kind);
        if self.at(Punct::Semi) {
            self.prohibit_friend_lead_attrs(&mut ds);
            self.bump();
            self.act_on_member_decl_spec_only(&ds);
            return;
        }
        if ds.is_empty() && self.at_member_without_type() {
            self.member_without_type();
            return;
        }
        let is_static = matches!(ds.storage, Some((StorageSpec::Static, _)));
        let is_typedef = ds.is_typedef();
        let ds_range = Span::new(ds.lead_cxx11.unwrap_or(ds.loc), self.prev_end());
        let mut base_ty: Option<QualType> = None;
        let mut restrict_error = None;
        let mut first = true;
        let mut group = AutoGroup::default();
        loop {
            let mut d = Declarator::new(DeclaratorContext::Member, self.loc());
            d.friend = ds.friend.is_some();
            d.typedef = is_typedef;
            d.typed = ds.has_type_specifier();
            self.late_fn = LateFnParts {
                delay_args: !is_typedef,
                // A friend function's noexcept-specifier is a
                // complete-class context too ([class.mem.general]/7), as
                // in clang 19, unless it names a member of another class.
                delay_noexcept: !is_typedef,
                ..LateFnParts::default()
            };
            if !self.at(Punct::Colon) {
                self.parse_declarator(&mut d);
                self.parse_trailing_requires(&mut d);
                self.set_declarator_end(&mut d);
            }
            let mut parts = mem::take(&mut self.late_fn);
            let base = *base_ty.get_or_insert_with(|| self.member_base_type(&ds, &d));
            self.repeat_member_declspec_errors(&ds, &d, first, &mut restrict_error);
            let is_fn = d.is_function_declarator();
            let (has_width, width_failed) =
                self.parse_member_declarator_tail(&mut d, ds.friend.is_some());
            // [dcl.contract.func]/1: after the virt-specifiers; a
            // complete-class context ([class.mem.general]/7).
            parts.contracts = if is_fn { self.capture_member_contract_specs() } else { None };
            if !d.has_name() && !has_width {
                if !d.invalid {
                    self.error_expected_member_name(ds_range);
                }
                self.skip_until(&[Punct::RBrace], true, false);
                if first {
                    self.eat(Punct::Semi);
                    return;
                }
                break;
            }
            if first && is_fn && self.at_function_definition() {
                d.defaulted = self.at_eq_default();
                self.parse_member_function_definition(&ds, base, &d, parts);
                return;
            }
            if first {
                self.prohibit_friend_lead_attrs(&mut ds);
            }
            // A member variable template specialization is static ([temp.expl.spec]/3).
            let is_static = is_static || d.spec_id.is_some();
            let data_member = !is_static && !is_typedef && ds.friend.is_none();
            let (nsdmi, init) = self.classify_member_init(&mut d, has_width, is_fn, data_member);
            let (decl, target) =
                if width_failed { (None, None) } else { self.act_on_member(&ds, base, &d) };
            self.record_declarator_end(target, &d);
            self.check_friend_without_contracts(is_fn, &ds, &parts, target, &d);
            if let Some(x) = decl {
                self.s.act_on_group_declarator(&mut group, x);
            }
            if nsdmi {
                self.save_member_nsdmi(d.is_array_of_unknown_bound(), decl);
            } else {
                self.parse_member_init_in_place(&d, decl, init, is_static);
            }
            if !parts.is_empty()
                && let Some(f) = target
            {
                self.push_late(LateItem::Method { f, parts });
            }
            if !self.next_declarator(&mut tmpl_group) {
                break;
            }
            first = false;
        }
        self.s.act_on_end_group(group, &ds);
        if self.eat(Punct::Semi).is_none() {
            self.s.error(DiagId::parse_expected_semi_decl_list, self.expected_loc());
            self.skip_until(&[Punct::RBrace], true, false);
            self.eat(Punct::Semi);
        }
    }

    /// clang's in-class initializer classification of a member
    /// declarator `d` at `=` or `{`: (a default member initializer of a
    /// non-static data member, an initializer parsed in place). A pure
    /// specifier is stored in `d`.
    fn classify_member_init(
        &mut self,
        d: &mut Declarator,
        has_width: bool,
        is_fn: bool,
        data_member: bool,
    ) -> (bool, bool) {
        if !self.at(Punct::Equal) && !self.at(Punct::LBrace) {
            return (false, false);
        }
        if has_width && !d.has_name() {
            self.error_here(DiagId::parse_anon_bitfield_member_init);
            self.skip_until(&[Punct::Comma], true, false);
            (false, false)
        } else if is_fn {
            if let Some(z) = self.try_pure_specifier() {
                d.pure = Some(z);
                (false, false)
            } else {
                (false, true)
            }
        } else {
            d.init_style = Some(if self.at(Punct::Equal) { InitStyle::C } else { InitStyle::List });
            (data_member, !data_member)
        }
    }

    /// True at `= default`.
    pub(crate) fn at_eq_default(&self) -> bool {
        self.at(Punct::Equal) && self.kw_of(self.peek(1)) == Some(Kw::Default)
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
        // A friend definition: sema checks where it may be defined
        // ([class.friend]/6).
        let friend = ds.friend.is_some();
        if friend {
            self.s.act_function_def_next(true);
        }
        let (_, target) = self.act_on_member(ds, base, d);
        if friend {
            self.s.act_function_def_next(false);
            self.check_friend_without_contracts(true, ds, &parts, target, d);
        }
        let mut parts = parts;
        // [dcl.contract.res]/2: a deduced return type is known after the
        // body.
        let late_contracts = match (target, parts.contracts.take()) {
            (Some(f), Some((t, ContractDeclKind::Definition)))
                if self.s.contract_return_undeduced(f) =>
            {
                Some(t)
            }
            (_, c) => {
                parts.contracts = c;
                None
            }
        };
        if !parts.is_empty()
            && let Some(f) = target
        {
            self.push_late(LateItem::Method { f, parts });
        }
        if self.at(Punct::Equal) {
            // clang `ParseCXXInlineMethodDef`: an in-class `= default` /
            // `= delete` definition ends at the last character of the
            // keyword (`setRangeEnd(KWEndLoc)`), where an implicit
            // assignment body is located.
            let kw = self.peek(1).span;
            let end = if kw.lo.is_file() && kw.hi.to_raw() > kw.lo.to_raw() {
                kw.lo.with_offset(kw.hi.to_raw() - kw.lo.to_raw() - 1)
            } else {
                d.end
            };
            self.record_fn_end(target, end);
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
            self.s.act_on_delayed_body(f);
            self.push_late(LateItem::Body { f, ctor, toks: LateToks { toks, end } });
            if let Some(toks) = late_contracts {
                self.push_late(LateItem::Contracts { f, toks });
            }
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
        if let Some(level) = self.member_depr.take() {
            self.s.end_delayed_deprecation_to(level, decl);
        }
        let target = decl.and_then(|x| {
            let decls = &self.s.ast.decls;
            if decls.get(x).kind() == DeclKind::Friend {
                match decls.friend_target(x) {
                    // A friend function template's body is its pattern's.
                    FriendTarget::Decl(t) => {
                        let t = self.s.ast.template_info(t).and_then(|i| i.pattern).unwrap_or(t);
                        decls.function(t).map(|_| t)
                    }
                    FriendTarget::Type { .. } => None,
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
            (None, Some((SpecialName::DeductionGuide(_), _))) => "<deduction guide>".to_owned(),
            (None, Some((SpecialName::LiteralOperator(n), _))) => {
                format!("operator\"\"{}", sym(*n))
            }
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
        let (i, last) = self.scan_qualifier();
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
        if i == self.pos() && self.deduction_guide_at(i).is_some() {
            return true;
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

    /// The class template whose deduction-guide ([temp.deduct.guide])
    /// starts at token `i`: an unqualified template-name of a class
    /// template, a parenthesized parameter list, then `->` (clang's
    /// `isDeductionGuideName` plus its declarator check). The
    /// injected-class-name in its own class is a constructor instead.
    pub(crate) fn deduction_guide_at(&self, i: usize) -> Option<DeclId> {
        let n = self.ident_of(self.tok_at(i))?;
        if Self::punct_of(self.tok_at(i + 1)) != Some(Punct::LParen) {
            return None;
        }
        let NameClass::TypeTemplate(t) = self.s.classify_name(n, None) else { return None };
        if self.s.ast.decls.get(t).kind() != DeclKind::ClassTemplate
            || self.s.names_injected_class(t)
        {
            return None;
        }
        let close = self.skip_balanced(i + 1)?;
        (Self::punct_of(self.tok_at(close)) == Some(Punct::Arrow)).then_some(t)
    }

    // -- special declarator-ids ----------------------------------------------------------

    /// Scans the nested-name-specifier starting here, of identifiers and
    /// simple template-ids (`::A::B<int>::`): the index of the token after
    /// it and its last name (`B`), so that `C<T>::C(` and `C<T>::~C(` are
    /// recognised as constructor and destructor declarators
    /// ([class.qual]/2).
    fn scan_qualifier(&self) -> (usize, Option<Symbol>) {
        let mut i = self.pos();
        if Self::punct_of(self.tok_at(i)) == Some(Punct::ColonColon) {
            i += 1;
        }
        let mut last = None;
        while let Some(n) = self.ident_of(self.tok_at(i)) {
            match Self::punct_of(self.tok_at(i + 1)) {
                Some(Punct::ColonColon) => i += 2,
                Some(Punct::Less) => match self.skip_template_args(i + 1) {
                    Some((close, false))
                        if Self::punct_of(self.tok_at(close + 1)) == Some(Punct::ColonColon) =>
                    {
                        i = close + 2;
                    }
                    _ => break,
                },
                _ => break,
            }
            last = Some(n);
        }
        (i, last)
    }

    /// The last name in the nested-name-specifier starting here (`B` in
    /// `A::B::f`), for recognising `C::C` ([class.ctor.general]/1).
    pub(crate) fn last_qualifier_name(&self) -> Option<Symbol> {
        self.scan_qualifier().1
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
    /// CWG2521 ([over.literal]/1, [depr.lit]): the suffix identifier of
    /// `operator "" _x`, preceded by whitespace, is deprecated; clang 22
    /// warns by default (`-fclang-compat=22` only).
    fn warn_spaced_literal_suffix(&mut self) {
        let t = self.tok();
        if self.s.clang_compat >= 22
            && (t.has(TokFlags::LEADING_SPACE) || t.has(TokFlags::START_OF_LINE))
        {
            let name = self.spelling_of(&t);
            let m = self.s.diag(DiagId::sema_deprecated_literal_operator_id, t.span.lo).arg(name);
            self.s.emit(m);
        }
    }

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
            if self.at(Punct::Less) && d.friend && !self.s.has_pending_head() {
                // `friend bool operator==<>(const X&, const X&);`
                // ([temp.friend]/1.3).
                let t = self.s.friend_spec_template(DeclName::operator(op), d.qualifier);
                let Some((args, _, r)) = self.parse_template_arg_list() else {
                    d.invalid = true;
                    return;
                };
                if let Some(t) = t {
                    d.spec_id = self.s.act_on_specialization_declarator_id(t, &args, loc, r);
                } else {
                    let n = DeclName::operator(op);
                    self.s.act_on_friend_spec_without_template(n, d.qualifier, loc);
                    d.dropped = true;
                }
                return;
            }
            if self.at(Punct::Less)
                && (self.s.has_explicit_spec_head() || self.s.has_explicit_inst_head())
                && let Some(t) = self.s.operator_function_template(op, d.qualifier)
            {
                // `template<> R operator==<int>(...)` ([temp.expl.spec]).
                let Some((args, _, r)) = self.parse_template_arg_list() else {
                    d.invalid = true;
                    return;
                };
                d.spec_id = self.s.act_on_specialization_declarator_id(t, &args, loc, r);
            }
            return;
        }
        if matches!(self.tok().kind, TokKind::StringLit(_) | TokKind::RawStringLit(_)) {
            // literal-operator-id ([over.literal]/1): `operator "" X` or
            // `operator ""X` (clang `ParseUnqualifiedIdOperator`).
            // Adjacent string literals concatenate ([lex.string]/7).
            let first = self.loc();
            let mut empty = true;
            let mut ud = None;
            while self.at_string() {
                let t = self.bump();
                let sp = self.spelling_of(&t);
                let lit = kiln_lex::literal::decode_string(&sp, self.lit_opts);
                empty &= lit.units.is_empty();
                if (lit.encoding != kiln_lex::Encoding::Ordinary || lit.raw) && !d.invalid {
                    self.s.error(DiagId::parse_literal_operator_string_prefix, t.span.lo);
                    d.invalid = true;
                }
                if let Some(u) = lit.ud_suffix {
                    ud.get_or_insert_with(|| self.s.intern(u));
                }
            }
            if !empty && !d.invalid {
                self.s.error(DiagId::parse_literal_operator_string_not_empty, first);
                d.invalid = true;
            }
            let suffix = if ud.is_some() {
                ud
            } else if let Some(n) = self.ident() {
                self.warn_spaced_literal_suffix();
                self.bump();
                Some(n)
            } else {
                let m = self.s.diag(DiagId::parse_expected, self.expected_loc()).arg("identifier");
                self.s.emit(m);
                None
            };
            match suffix {
                Some(n) if !d.invalid => {
                    d.special = Some((SpecialName::LiteralOperator(n), loc));
                }
                _ => d.invalid = true,
            }
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
        cd.site = DeclSite::ConversionId;
        self.parse_conversion_declarator(&mut cd);
        let base = self.s.type_from_declspec(&ds);
        let ty = self.s.type_for_declarator_at(base, ds.type_loc.unwrap_or(ds.loc), &cd);
        if ds.invalid || cd.invalid {
            d.invalid = true;
        }
        d.special = Some((SpecialName::Conversion(ty), loc));
        d.conv_type_loc = if ds.auto_constraint.is_some() && ds.constraint_loc.is_valid() {
            ds.constraint_loc
        } else {
            ds.type_loc.unwrap_or(ds.loc)
        };
    }

    /// conversion-declarator ([class.conv.fct]/1): ptr-operators only,
    /// taken greedily.
    fn parse_conversion_declarator(&mut self, d: &mut Declarator) {
        let loc = self.loc();
        if let Some(star) = self.at_member_pointer() {
            self.parse_member_pointer(d, star, Self::parse_conversion_declarator);
            return;
        }
        match Self::punct_of(self.tok()) {
            Some(Punct::Star) => {
                self.bump();
                let (quals, nulls) = self.parse_ptr_quals();
                self.parse_conversion_declarator(d);
                d.push_chunk(Chunk::Pointer { quals, loc }, nulls);
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
                Some(mut b) => {
                    self.s.check_base_annotation_packs(&mut b.attrs, b.pack_expansion);
                    bases.push(b);
                }
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
            // An annotation operand may name the pack of a `B...`
            // (checked by the caller once the `...` is read; WP-867).
            let saved = self.s.set_base_annotation_packs(true);
            self.parse_attributes(&mut attrs);
            self.s.set_base_annotation_packs(saved);
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
        // (WP-074). `decltype(auto)` is not a class-or-decltype. A
        // `decltype(e)::` qualifies a class-name (WP-119).
        if self.decltype_scope_end(self.pos()).is_some() && !self.annotate_decltype_scope() {
            return None;
        }
        if self.at_decltype_scope() && !self.decltype_scope_ok() {
            // clang goes on to the name after the failed qualifier.
            self.bump();
            self.error_here(DiagId::parse_expected_class_name);
            return None;
        }
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
            let scanned =
                self.scan_name_at(self.pos()).and_then(|(end, split, ..)| (!split).then_some(end));
            let Some(ty) = self.parse_template_type_name(DeclaratorContext::TypeName) else {
                // A qualifier that fails (`X<A>::B` where `X<A>` fails
                // to instantiate here): clang goes on to the name after
                // it, "expected class name".
                if let Some(end) = scanned
                    && self.pos() < end
                    && self.ident_of(self.tok_at(end - 1)).is_some()
                    && Self::punct_of(self.tok_at(end - 2)) == Some(Punct::ColonColon)
                {
                    while self.pos() < end - 1 {
                        self.bump();
                    }
                    self.error_here(DiagId::parse_expected_class_name);
                }
                return None;
            };
            let pack_expansion = self.eat(Punct::Ellipsis).is_some();
            return Some(BaseSpecInfo { ty, loc, is_virtual, access, pack_expansion, attrs });
        }
        let Ok(qual) = self.parse_nested_name_nns() else {
            return None;
        };
        let q = qual.map(|q| q.ctx);
        let Some(name) = self.ident() else {
            self.error_here(DiagId::parse_expected_class_name);
            return None;
        };
        let id_loc = self.bump().span.lo;
        // The class-name of a base-specifier is a type-only lookup
        // ([class.derived.general]/2): a function of the same scope does
        // not hide the class (clang's `getTypeName` with `isClassName`
        // looks it up as a nested-name-specifier name).
        let NameClass::Type(ty, d) = self.s.classify_nested_name(name, q) else {
            // `N::B` with `N` a member class of the current instantiation
            // that has a dependent base: a member of an unknown
            // specialization ([temp.dep.type]/6), looked up at
            // instantiation (oneTBB's `const_accessor : node::scoped_type`;
            // WP-803).
            if let Some(ty) = qual.and_then(|qual| {
                self.s.act_unknown_specialization_type(qual, name, TagKeyword::None, id_loc)
            }) {
                let pack_expansion = self.eat(Punct::Ellipsis).is_some();
                return Some(BaseSpecInfo { ty, loc, is_virtual, access, pack_expansion, attrs });
            }
            self.s.error(DiagId::parse_expected_class_name, id_loc);
            return None;
        };
        self.s.check_type_name_access(name, q, d, id_loc);
        let ty = self.named_type(ty, d);
        // `Ts...[i]` (C++26 [dcl.type.pack.index]: a computed-type-specifier
        // is a class-or-decltype).
        let ty = match self.parse_pack_index(id_loc) {
            Ok(Some((ellipsis, index, _))) => {
                self.s.act_pack_indexing_type_named(ty, name, id_loc, ellipsis, index)
            }
            Ok(None) => ty,
            Err(()) => return None,
        };
        if self.s.is_error_type(ty) {
            return None;
        }
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
                | Kw::Nonnull
                | Kw::Nullable
                | Kw::NullUnspecified
                | Kw::NullableResult
                | Kw::Inline
                | Kw::Virtual
                | Kw::Friend
                | Kw::Noreturn
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
                        | Kw::Float16
                        | Kw::Float32
                        | Kw::Float64
                        | Kw::StdFloat128
                        | Kw::Float32x
                        | Kw::Float64x
                        | Kw::Bf16
                        | Kw::Fp16
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
