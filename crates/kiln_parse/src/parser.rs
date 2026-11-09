//! The parser state, token access and error recovery (spec/parse.md §3).

use std::cell::{Cell, RefCell};

use kiln_ast::decl::DeclKind;
use kiln_ast::types::UnaryTransformKind;
use kiln_ast::{AttrId, DeclId, ExprId, NnsId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_lex::literal::LiteralOptions;
use kiln_lex::{LexOptions, Punct, Std, TokFlags, TokKind, Token};
use kiln_sema::using::Qualifier;
use kiln_sema::{NameClass, Sema};
use kiln_source::symbol::Symbol;
use kiln_source::{Loc, SourceManager, Span};

use crate::class::LateClass;
use crate::keywords::{Keywords, Kw};
use crate::tentative::TypeIdContext;

use crate::tokens::TokenSource;

/// The result of parsing an expression: `None` when a syntax error was
/// reported (clang's invalid `ExprResult`); the caller recovers by
/// skipping. Semantic errors produce an error expression instead.
pub type PExpr = Option<ExprId>;

/// Whether a dependent qualified name in the parameter-declaration being
/// parsed is implicitly a type ([temp.res.general]/4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ParamTypename {
    /// It is (a member, friend, qualified or lambda declarator, ...).
    Implicit,
    /// It is not: a missing `typename` is diagnosed.
    Required,
    /// It was not, and a missing `typename` was diagnosed: the function
    /// declaration is invalid (clang), so a later out-of-line definition
    /// does not match it.
    Missing,
}

/// The bracket kinds whose nesting [`Parser::bump`] tracks.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Bracket {
    Paren = 0,
    Square = 1,
    Brace = 2,
}

/// A saved token buffer being re-parsed (spec/parse.md §2.4): while one
/// is active, [`Parser::tok_at`] and [`Parser::bump`] read its tokens
/// (indices from 0) and then an `Eof` sentinel, and the state of the
/// interrupted stream is restored by [`Parser::end_replay`].
///
/// An `inject` frame instead holds tokens inserted before the current
/// one (clang's `EnterToken`, [`Parser::inject_semi`]): after them the
/// interrupted stream continues, and the frame pops itself once they are
/// consumed.
struct Replay {
    toks: Vec<Token>,
    eof: Token,
    pos: usize,
    cur: Token,
    prev_hi: Loc,
    prev_lo: Loc,
    depth: [u32; 3],
    /// The interrupted stream's `greater_is_op` and `greater_off_depth`.
    greater: (bool, u32),
    inject: bool,
}

/// A recursive-descent parser over the preprocessor's token stream,
/// driving [`Sema`].
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent parse-state flags, as clang's Parser"
)]
pub struct Parser<'a, 'sm> {
    /// Semantic analysis.
    pub(crate) s: Sema<'a, 'sm>,
    /// The sources (for literal spellings and diagnostics).
    pub(crate) sm: &'sm SourceManager,
    /// The token stream; lookahead (`&self`) fills its window lazily.
    src: RefCell<TokenSource<'a, 'sm>>,
    /// The token at `pos`.
    cur: Token,
    /// The end of the token before `pos`.
    prev_hi: Loc,
    /// The start of the token before `pos`.
    prev_lo: Loc,
    pos: usize,
    /// The start of the last `__attribute__((...))`'s closing `)`, or
    /// `Loc::INVALID` (a member declarator's end, clang `SetRangeEnd`).
    pub(crate) gnu_attr_end: Loc,
    /// The GNU declaration attributes of the last ptr-operator's
    /// attribute positions (`int *__attribute__((may_alias)) p`): clang
    /// applies them to the declaration (`ProcessDeclAttributes` walks the
    /// declarator chunks).
    pub(crate) chunk_decl_attrs: Vec<AttrId>,
    /// The last run of adjacent `[[...]]` specifiers: the start of the
    /// token before it and of its final `]` (a declarator's end excludes
    /// them, clang `ParseFunctionDeclarator`).
    pub(crate) cxx11_attr_run: (Loc, Loc),
    /// Open `(`, `[`, `{` consumed and not yet closed (clang's
    /// `ParenCount`/`BracketCount`/`BraceCount`).
    depth: [u32; 3],
    pub(crate) kw: Keywords,
    pub(crate) lex_opts: LexOptions,
    pub(crate) lit_opts: LiteralOptions,
    pub(crate) std: Std,
    /// Active token replays, innermost last.
    replay: Vec<Replay>,
    /// Module state, once modules are active (spec/modules.md).
    pub(crate) mods: Option<Box<crate::modules::ParseModules>>,
    /// The classes being defined whose complete-class contexts are saved
    /// for replay (clang's `ParsingClassStack`), innermost last.
    pub(crate) late: Vec<LateClass>,
    /// The saved default arguments and *noexcept-specifier* of the member
    /// function declarator being parsed, taken by the member-declaration.
    pub(crate) late_fn: crate::class::LateFnParts,
    /// The deprecation delay level of the member-declaration being
    /// parsed, closed by its first declarator ([`Sema::begin_delayed_deprecation`]).
    pub(crate) member_depr: Option<usize>,
    /// The `act_on_*` calls the class parser made, in order (for the
    /// sequence tests of spec/sema-class.md §2.1).
    #[cfg(test)]
    pub(crate) trace: Vec<String>,
    /// The next statement parsed is directly in a statement expression's
    /// compound statement (clang's `ParsedStmtContext::InStmtExpr`); taken
    /// by `parse_statement`, passed on through labels.
    pub(crate) in_stmt_expr: bool,
    /// The expression being parsed is invalid as clang's `ExprError`
    /// though the parser goes on past it: a call whose `)` is missing
    /// after a delayed typo (clang's `ParsePostfixExpressionSuffix` sets
    /// `LHS = ExprError()`). An expression statement then skips to its
    /// `;` with no "expected ';'" (`parse_expression_statement`).
    pub(crate) expr_invalid: bool,
    /// The `(` of a C-style cast's parenthesized operand, `(T)(a, b)`:
    /// clang parses it as a `ParenListExpr` whose commas are built at
    /// the `(` (`MaybeConvertParenListExprToParenExpr`).
    pub(crate) cast_paren_list: Option<Loc>,
    /// The innermost parenthesized expression being parsed that is a
    /// call's callee, `(e)(args)` or `((e))(args)`: the token index of
    /// its first inner token and of its `)`. A function template-id that
    /// is the whole of it stays an overload set for the call to resolve
    /// ([`Parser::template_id_is_callee`]).
    pub(crate) paren_callee: Option<(usize, usize)>,
    /// The `(` of a C-style cast's parenthesized operand for the next
    /// [`Parser::parse_rhs`] (taken at its entry): its top-level commas
    /// are [`Parser::parse_paren_list_rest`]'s.
    pub(crate) comma_loc: Option<Loc>,
    /// False while parsing a template argument: a `>` or `>>` ends the
    /// argument instead of being an operator ([temp.names]/4, clang's
    /// `GreaterThanIsOperator`). Brackets restore it: see
    /// [`Parser::greater_ends_arg`].
    pub(crate) greater_is_op: bool,
    /// The total bracket depth where `greater_is_op` was last turned off:
    /// a `>` nested in a bracket opened since is an operator again.
    pub(crate) greater_off_depth: u32,
    /// The parameter-declaration being parsed declares a type with an
    /// unexpanded pack (or `auto`): a `...` before its `)` belongs to the
    /// declarator, not the parameter list ([dcl.fct]/14).
    pub(crate) pack_hint: bool,
    /// The token position just past the last unary `&`: an id-expression
    /// starting there is the operand of `&` (clang's `isAddressOfOperand`).
    pub(crate) addr_operand_at: Option<usize>,
    /// The declarator being parsed is a handler's exception-declaration,
    /// whose direct declarator takes a misplaced `...` (clang
    /// `ParseDirectDeclarator` consumes one wherever a name may follow;
    /// sema rejects it). Taken by the first direct declarator.
    pub(crate) catch_declarator: bool,
    /// A handler's exception-declaration is being parsed (clang does not
    /// run `CheckExtraCXXDefaultArguments` on it, [dcl.fct.default]/3).
    pub(crate) in_exception_decl: bool,
    /// The type-constraint location of the constrained placeholder the
    /// last type-id contained (its decl-specifier's, or one in its
    /// trailing return types), for the enclosing declarator's
    /// [`Declarator::trailing_constraint`]; invalid when none.
    pub(crate) type_id_constraint: Loc,
    /// The type-id about to be parsed is a named cast's (clang builds it
    /// without `ActOnTypeName`: no ignored-`mode` warning).
    pub(crate) named_cast_type_id: bool,
    /// The decl-specifier-seq of a parameter-declaration being parsed is
    /// not one where a dependent qualified name is implicitly a type
    /// ([temp.res.general]/4: a parameter of a function declared at
    /// namespace or block scope with an unqualified declarator-id), so a
    /// missing `typename` there is diagnosed.
    pub(crate) param_typename: ParamTypename,
    /// The parameter-declaration whose decl-specifier-seq is being parsed
    /// is the first of a declarator clang disambiguates tentatively
    /// (`Some(true)`) or another one (`Some(false)`); `None` outside one.
    /// Selects clang's recovery for an unqualified `typename T`
    /// (spec/parse.md, typename-specifiers).
    pub(crate) param_recovery: Option<bool>,
    /// The [`kiln_sema::TagSite`] of the next decl-specifier-seq, when its
    /// caller knows more than its [`kiln_sema::DeclaratorContext`] (a
    /// non-type template parameter, `sizeof`, a template argument, an
    /// alias-declaration...): taken by
    /// [`Parser::parse_decl_specifiers_from`], so nested ones get their own
    /// (spec/sema.md §4.4.1).
    pub(crate) next_tag_site: Option<kiln_sema::TagSite>,
    /// Set when an unqualified `typename` ended a parameter's
    /// decl-specifier-seq without consuming its name: the parameter list
    /// stops there (clang's "expected ')'").
    pub(crate) param_abort: bool,
    /// Set when `typename identifier` failed (no qualifier) and the
    /// identifier names no type: the identifier is left unconsumed and
    /// the decl-specifier-seq ends there (clang's `DoneWithDeclSpec`).
    pub(crate) typename_failed: bool,
    /// The name and end of the last `typename T...[i]` whose pack index
    /// was consumed after "expected a qualified name after 'typename'".
    pub(crate) typename_pack_index: Option<(Loc, Loc)>,
    /// Memoized [`Parser::skip_template_args`] results (spec/parse.md
    /// §2.5): parsing `A<B<C<...>>>` scans each nested list once, not
    /// once per enclosing level.
    targs_memo: RefCell<TemplateArgsMemo>,
    /// Set when the stack guard tripped (spec/sema-templates.md P4 (c)):
    /// every token read from then on is this `Eof`, so every rule
    /// unwinds.
    halt: Cell<Option<Token>>,
    /// The parsed `decltype ( expression )` whose `::` is the current
    /// token: the first component of a nested-name-specifier (WP-119,
    /// clang's `annot_decltype`). Cleared by every [`Parser::bump`].
    decltype_scope: Option<DecltypeScope>,
    /// The access delay level a template head opened for the
    /// declaration after it ([class.access]/7), taken by
    /// `parse_simple_declaration`.
    pub(crate) template_delay_level: Option<usize>,
    /// The kind of template declaration whose declaration is parsed
    /// next, taken by `parse_simple_declaration` and
    /// `parse_member_declaration`: a later declarator is "a template
    /// declaration can only declare a single entity" ([temp.pre]/5).
    pub(crate) template_decl_kind: Option<crate::templ::TemplateDeclKind>,
    /// The leading attribute-specifier-seq of a statement, parsed before
    /// it is known to be a declaration: the declaration's decl-specifiers
    /// take it ([dcl.pre]/5, [dcl.attr.grammar]/5).
    pub(crate) stmt_attrs: Vec<AttrId>,
    /// The attributes `parse_decl_specifiers` starts its seq with (a
    /// declaration statement's leading ones).
    pub(crate) lead_decl_attrs: Vec<AttrId>,
    /// Where `stmt_attrs` / `lead_decl_attrs` start (`[[` or `alignas`).
    pub(crate) stmt_attrs_loc: Option<Loc>,
    /// Pending loop-hint pragmas, by the location of the token starting
    /// the statement they apply to (spec/parse.md §2.3).
    pub(crate) loop_hints: Vec<(Loc, Vec<crate::tokens::LoopHintInfo>)>,
    /// The FP pragmas not yet taken, keyed by the token they precede
    /// (spec/parse.md §2.3).
    pub(crate) fp_pragmas: Vec<(Loc, crate::tokens::FpPragma)>,
}

/// A `decltype(e)` parsed ahead as a nested-name-specifier
/// ([`Parser::annotate_decltype_scope`]).
#[derive(Clone, Copy, Debug)]
struct DecltypeScope {
    /// The replay depth and token index of the `::`.
    level: usize,
    pos: usize,
    /// The scope and specifier; `None` after an error (diagnosed).
    qual: Option<(DeclId, NnsId)>,
    /// The `decltype` keyword.
    loc: Loc,
}

/// The lookahead scan memo: the template-argument scan by its `<` index
/// and the name scan by its first token's index → the scan's result,
/// valid while [`Sema::lookup_stamp`] is `stamp` (name classification
/// unchanged) and the token stream is unchanged (cleared when a replay
/// starts or ends and when [`Parser::split_greater`] rewrites a token).
#[derive(Default)]
struct TemplateArgsMemo {
    stamp: u64,
    map: kiln_source::fx::FxHashMap<usize, Option<(usize, bool)>>,
    /// [`Parser::scan_name_walk`]'s results (WP-919), at most
    /// [`NAME_MEMO_MAX`] of them.
    names: kiln_source::fx::FxHashMap<usize, Result<ScannedName, ScanStop>>,
}

/// The bound on the name-scan memo: it is emptied when full.
const NAME_MEMO_MAX: usize = 4096;

impl TemplateArgsMemo {
    /// Empties the memo unless its results are valid under `stamp`.
    fn refresh(&mut self, stamp: u64) {
        if self.stamp != stamp {
            self.stamp = stamp;
            self.map.clear();
            self.names.clear();
        }
    }

    /// The template-argument scans valid under `stamp`.
    fn valid_for(
        &mut self,
        stamp: u64,
    ) -> &mut kiln_source::fx::FxHashMap<usize, Option<(usize, bool)>> {
        self.refresh(stamp);
        &mut self.map
    }
}

impl std::fmt::Debug for Parser<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Parser").field("pos", &self.pos).finish_non_exhaustive()
    }
}

impl<'a, 'sm> Parser<'a, 'sm> {
    /// Hands Sema the preprocessor's attribute-macro history (for the
    /// `-Wimplicit-fallthrough` fix-it spelling) when it changed; called
    /// before a function or lambda body is finished.
    pub(crate) fn sync_attr_macros(&mut self) {
        let src = self.src.get_mut();
        let (generation, macros) = src.preprocessor().attr_macros();
        self.s.sync_attr_macros(generation, || {
            macros
                .iter()
                .map(|m| kiln_sema::AttrMacro {
                    name: m.name.clone(),
                    def: m.def,
                    end: m.end,
                    tokens: m.tokens.clone(),
                })
                .collect()
        });
    }

    /// A parser over `src`. `s`'s AST must use the preprocessor's
    /// interner.
    pub fn new(
        mut s: Sema<'a, 'sm>,
        mut src: TokenSource<'a, 'sm>,
        lit_opts: LiteralOptions,
    ) -> Self {
        let lex_opts = src.lex_options();
        let std = lex_opts.std;
        let cxx11 = std >= Std::Cxx11;
        let cxx26 = std >= Std::Cxx26;
        let mut kw = Keywords::with_std(s.ast.symbols(), s.gnu, cxx11, cxx26, s.clang_compat);
        if !lit_opts.char8() {
            kw.remove(s.ast.symbols(), "char8_t");
        }
        s.set_std(std);
        let sm = src.source_manager();
        let cur = src.get(0);
        let mut p = Parser {
            s,
            sm,
            src: RefCell::new(src),
            cur,
            prev_hi: Loc::INVALID,
            prev_lo: Loc::INVALID,
            pos: 0,
            gnu_attr_end: Loc::INVALID,
            chunk_decl_attrs: Vec::new(),
            cxx11_attr_run: (Loc::INVALID, Loc::INVALID),
            depth: [0; 3],
            kw,
            lex_opts,
            lit_opts,
            std,
            replay: Vec::new(),
            mods: None,
            late: Vec::new(),
            late_fn: crate::class::LateFnParts::default(),
            member_depr: None,
            #[cfg(test)]
            trace: Vec::new(),
            in_stmt_expr: false,
            expr_invalid: false,
            cast_paren_list: None,
            paren_callee: None,
            comma_loc: None,
            greater_is_op: true,
            greater_off_depth: 0,
            pack_hint: false,
            addr_operand_at: None,
            catch_declarator: false,
            in_exception_decl: false,
            type_id_constraint: Loc::INVALID,
            named_cast_type_id: false,
            param_typename: ParamTypename::Implicit,
            param_recovery: None,
            next_tag_site: None,
            param_abort: false,
            typename_failed: false,
            typename_pack_index: None,
            targs_memo: RefCell::new(TemplateArgsMemo::default()),
            halt: Cell::new(None),
            decltype_scope: None,
            template_delay_level: None,
            template_decl_kind: None,
            stmt_attrs: Vec::new(),
            lead_decl_attrs: Vec::new(),
            stmt_attrs_loc: None,
            loop_hints: Vec::new(),
            fp_pragmas: Vec::new(),
        };
        if p.src.get_mut().module_lines() {
            p.module_activate();
        }
        p.handle_events(0);
        p
    }

    /// Finishes parsing (handling the events after the last token),
    /// returning the semantic state and the token source.
    pub fn finish(mut self) -> (Sema<'a, 'sm>, TokenSource<'a, 'sm>) {
        self.handle_events(usize::MAX);
        (self.s, self.src.into_inner())
    }

    /// Finishes parsing, returning the semantic state.
    pub fn into_sema(self) -> Sema<'a, 'sm> {
        self.finish().0
    }

    /// Handles the preprocessor events before token `pos`.
    fn handle_events(&mut self, pos: usize) {
        while let Some(e) = self.src.get_mut().take_event(pos) {
            self.on_event(e);
        }
    }

    // -- token access -----------------------------------------------------------

    /// The index of the current token in the stream.
    pub(crate) fn token_index(&self) -> usize {
        self.pos
    }

    /// Whether the parser is replaying saved tokens.
    pub(crate) fn in_replay(&self) -> bool {
        !self.replay.is_empty()
    }

    /// The current token.
    pub(crate) fn tok(&self) -> Token {
        self.halt.get().unwrap_or(self.cur)
    }

    /// The token `n` ahead of the current one (`Eof` past the end).
    pub(crate) fn peek(&self, n: usize) -> Token {
        self.tok_at(self.pos + n)
    }

    /// The token at absolute index `i` (`Eof` past the end; at most
    /// [`crate::tokens::KEEP_BEHIND`] tokens before the current one).
    pub(crate) fn tok_at(&self, i: usize) -> Token {
        if let Some(t) = self.halt.get() {
            return t;
        }
        if i == self.pos { self.cur } else { self.tok_in(self.replay.len(), i) }
    }

    /// The stack guard's check at a recursive rule (spec/sema-templates.md
    /// P4 (c)): false when the stack is nearly exhausted, after the fatal
    /// "kiln: stack exhausted during parsing", and from then on the
    /// parser reads only `Eof`. Also false once instantiation tripped the
    /// guard.
    #[inline]
    pub(crate) fn stack_ok(&self) -> bool {
        if self.halt.get().is_some() {
            return false;
        }
        if !self.s.stack_exhausted() && !self.s.stack_low() {
            return true;
        }
        self.halt_on_stack()
    }

    #[cold]
    #[inline(never)]
    fn halt_on_stack(&self) -> bool {
        let loc = self.loc();
        let _ = self.s.stack_ok(kiln_sema::stack_guard::StackPhase::Parsing, loc);
        let eof = Token { kind: TokKind::Eof, span: Span::new(loc, loc), flags: TokFlags::empty() };
        self.halt.set(Some(eof));
        false
    }

    /// The token at index `i` of the stream `level` replays deep (0 is the
    /// token source).
    fn tok_in(&self, level: usize, i: usize) -> Token {
        let Some(r) = level.checked_sub(1).and_then(|l| self.replay.get(l)) else {
            return self.src.borrow_mut().get(i);
        };
        match r.toks.get(i) {
            Some(t) => *t,
            None if r.inject => self.tok_in(level - 1, r.pos + (i - r.toks.len())),
            None => r.eof,
        }
    }

    /// The current index (for lookahead scans).
    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    /// The current token's location.
    pub(crate) fn loc(&self) -> Loc {
        self.tok().span.lo
    }

    /// The end of the previous token (where clang's `ExpectAndConsume`
    /// reports a missing token).
    pub(crate) fn prev_end(&self) -> Loc {
        if self.prev_hi == Loc::INVALID { self.loc() } else { self.prev_hi }
    }

    /// The start of the previous token (clang's `PrevTokLocation`, where
    /// a declarator's source range ends).
    pub(crate) fn prev_lo(&self) -> Loc {
        if self.prev_lo == Loc::INVALID { self.loc() } else { self.prev_lo }
    }

    /// True at the end of input.
    pub(crate) fn at_eof(&self) -> bool {
        self.tok().kind == TokKind::Eof
    }

    /// Consumes the current token (never `Eof`) and returns it.
    pub(crate) fn bump(&mut self) -> Token {
        let t = self.tok();
        if t.kind == TokKind::Eof {
            return t;
        }
        self.decltype_scope = None;
        if let TokKind::Punct(p) = t.kind {
            match p {
                Punct::LParen => self.depth[Bracket::Paren as usize] += 1,
                Punct::LSquare => self.depth[Bracket::Square as usize] += 1,
                Punct::LBrace => self.depth[Bracket::Brace as usize] += 1,
                Punct::RParen => self.close(Bracket::Paren),
                Punct::RSquare => self.close(Bracket::Square),
                Punct::RBrace => self.close(Bracket::Brace),
                _ => {}
            }
        }
        self.pos += 1;
        self.prev_hi = t.span.hi;
        self.prev_lo = t.span.lo;
        if let Some(r) = self.replay.last() {
            if r.inject && self.pos >= r.toks.len() {
                // The inserted tokens are consumed: resume the stream.
                if let Some(r) = self.replay.pop() {
                    self.pos = r.pos;
                    self.cur = r.cur;
                }
                self.forget_scans();
                return t;
            }
            self.cur = r.toks.get(self.pos).copied().unwrap_or(r.eof);
            return t;
        }
        let src = self.src.get_mut();
        src.release(self.pos);
        self.cur = src.get(self.pos);
        if src.has_event(self.pos) {
            self.handle_events(self.pos);
        }
        t
    }

    /// Starts re-parsing `toks` (spec/parse.md §2.4): the current token
    /// becomes `toks[0]`, and after the last one an `Eof` sentinel at
    /// `end` (the token that ended the saved range) stops every rule.
    /// Paired with [`Parser::end_replay`].
    pub(crate) fn begin_replay(&mut self, toks: Vec<Token>, end: Loc) {
        let eof = Token { kind: TokKind::Eof, span: Span::new(end, end), flags: TokFlags::empty() };
        let cur = toks.first().copied().unwrap_or(eof);
        let saved = Replay {
            toks,
            eof,
            pos: self.pos,
            cur: self.cur,
            prev_hi: self.prev_hi,
            prev_lo: self.prev_lo,
            depth: self.depth,
            greater: (self.greater_is_op, self.greater_off_depth),
            inject: false,
        };
        self.replay.push(saved);
        self.forget_scans();
        self.pos = 0;
        self.cur = cur;
        self.prev_hi = Loc::INVALID;
        self.prev_lo = Loc::INVALID;
        self.depth = [0; 3];
        self.greater_is_op = true;
        self.greater_off_depth = 0;
    }

    /// Inserts a `;` before the current token (at its location), as clang
    /// does to recover from a missing `;` after a class or enum definition
    /// (`PP.EnterToken` + `Tok.setKind(tok::semi)`).
    pub(crate) fn inject_semi(&mut self) {
        let at = self.loc();
        self.inject(TokKind::Punct(Punct::Semi), at);
    }

    /// Inserts a token of kind `kind` at `at` before the current token
    /// (clang's `UnconsumeToken` of a replacement token).
    pub(crate) fn inject(&mut self, kind: TokKind, at: Loc) {
        let semi = Token { kind, span: Span::new(at, at), flags: TokFlags::empty() };
        let saved = Replay {
            toks: vec![semi],
            eof: semi,
            pos: self.pos,
            cur: self.cur,
            prev_hi: self.prev_hi,
            prev_lo: self.prev_lo,
            depth: self.depth,
            greater: (self.greater_is_op, self.greater_off_depth),
            inject: true,
        };
        self.replay.push(saved);
        self.forget_scans();
        self.pos = 0;
        self.cur = semi;
    }

    /// Ends the innermost replay, restoring the interrupted stream, and
    /// returns true if the saved tokens were all consumed.
    pub(crate) fn end_replay(&mut self) -> bool {
        while self.replay.last().is_some_and(|r| r.inject) {
            self.replay.pop();
        }
        let done = self.at_eof();
        self.forget_scans();
        if let Some(r) = self.replay.pop() {
            self.pos = r.pos;
            self.cur = r.cur;
            self.prev_hi = r.prev_hi;
            self.prev_lo = r.prev_lo;
            self.depth = r.depth;
            (self.greater_is_op, self.greater_off_depth) = r.greater;
        }
        done
    }

    /// Records an `act_on_*` call for the sequence tests (a no-op
    /// outside tests).
    #[cfg_attr(not(test), allow(clippy::unused_self))]
    pub(crate) fn trace(&mut self, what: impl FnOnce(&Self) -> String) {
        #[cfg(test)]
        {
            let s = what(self);
            self.trace.push(s);
        }
        #[cfg(not(test))]
        let _ = what;
    }

    /// How many brackets of the kind `close` closes the parser has open
    /// (0 for a non-closer).
    pub(crate) fn depth_of(&self, close: Punct) -> u32 {
        let b = match close {
            Punct::RParen => Bracket::Paren,
            Punct::RSquare => Bracket::Square,
            Punct::RBrace => Bracket::Brace,
            _ => return 0,
        };
        self.depth[b as usize]
    }

    /// The number of brackets of every kind open at the current token.
    pub(crate) fn total_depth(&self) -> u32 {
        self.depth.iter().sum()
    }

    /// Whether a `>` here ends a template argument rather than being an
    /// operator ([temp.names]/4): not inside a bracket opened since the
    /// template argument began (clang's `BalancedDelimiterTracker`
    /// restores `GreaterThanIsOperator` inside `()`, `[]` and `{}`).
    pub(crate) fn greater_ends_arg(&self) -> bool {
        !self.greater_is_op && self.total_depth() <= self.greater_off_depth
    }

    fn close(&mut self, b: Bracket) {
        let d = &mut self.depth[b as usize];
        *d = d.saturating_sub(1);
    }

    /// The punctuator `t` is.
    pub(crate) fn punct_of(t: Token) -> Option<Punct> {
        match t.kind {
            TokKind::Punct(p) => Some(p),
            _ => None,
        }
    }

    /// True if the current token is `p`.
    pub(crate) fn at(&self, p: Punct) -> bool {
        self.tok().kind == TokKind::Punct(p)
    }

    /// True if the token `n` ahead is `p`.
    pub(crate) fn peek_is(&self, n: usize, p: Punct) -> bool {
        self.peek(n).kind == TokKind::Punct(p)
    }

    /// The keyword `t` is.
    pub(crate) fn kw_of(&self, t: Token) -> Option<Kw> {
        match t.kind {
            TokKind::Identifier(s) => self.kw.get(s),
            _ => None,
        }
    }

    /// The current token's keyword.
    pub(crate) fn kw(&self) -> Option<Kw> {
        self.kw_of(self.tok())
    }

    /// True if the current token is keyword `k`.
    pub(crate) fn at_kw(&self, k: Kw) -> bool {
        self.kw() == Some(k)
    }

    /// The builtin type trait or transform at token `i`: its name
    /// followed by `(` (spec/sema-templates.md §9).
    pub(crate) fn builtin_kw_at(&self, i: usize) -> Option<crate::keywords::BuiltinKw> {
        let TokKind::Identifier(s) = self.tok_at(i).kind else { return None };
        let b = self.kw.builtin(s)?;
        (self.tok_at(i + 1).kind == TokKind::Punct(Punct::LParen)).then_some(b)
    }

    /// The type transform (`__remove_cv(`...) at token `i`.
    pub(crate) fn transform_at(&self, i: usize) -> Option<UnaryTransformKind> {
        match self.builtin_kw_at(i)? {
            crate::keywords::BuiltinKw::Transform(k) => Some(k),
            crate::keywords::BuiltinKw::Trait(_) => None,
        }
    }

    /// The identifier (not a keyword) `t` is.
    pub(crate) fn ident_of(&self, t: Token) -> Option<Symbol> {
        match t.kind {
            TokKind::Identifier(s) if self.kw.get(s).is_none() => Some(s),
            _ => None,
        }
    }

    /// True if `t` is the contextual keyword `final` ([class.pre]/1,
    /// [class.mem.general]: an identifier with special meaning).
    pub(crate) fn is_final_tok(&self, t: Token) -> bool {
        t.kind == TokKind::Identifier(self.kw.final_)
    }

    /// True if `t` is the contextual keyword `override`.
    pub(crate) fn is_override_tok(&self, t: Token) -> bool {
        t.kind == TokKind::Identifier(self.kw.override_)
    }

    /// The current token as an identifier.
    pub(crate) fn ident(&self) -> Option<Symbol> {
        self.ident_of(self.tok())
    }

    /// Consumes `p` if it is next.
    pub(crate) fn eat(&mut self, p: Punct) -> Option<Loc> {
        if self.at(p) { Some(self.bump().span.lo) } else { None }
    }

    /// Consumes keyword `k` if it is next.
    pub(crate) fn eat_kw(&mut self, k: Kw) -> Option<Loc> {
        if self.at_kw(k) { Some(self.bump().span.lo) } else { None }
    }

    /// Consumes `p` or reports `expected 'p'` after the previous token
    /// (clang's `ExpectAndConsume`).
    pub(crate) fn expect(&mut self, p: Punct) -> Option<Loc> {
        if let Some(l) = self.eat(p) {
            return Some(l);
        }
        let d = self.s.diag(DiagId::parse_expected, self.expected_loc()).arg(quote(p));
        self.s.emit(d);
        None
    }

    /// Where a missing token is reported: after the previous token, or at
    /// the current one when that is the first token or at end of file
    /// (clang reports at the end of the previous token when it is valid).
    pub(crate) fn expected_loc(&self) -> Loc {
        self.prev_end()
    }

    /// Consumes the closing `close` of `open` at `open_loc`, or reports
    /// `expected 'close'` with `to match this 'open'`.
    pub(crate) fn expect_closing(&mut self, close: Punct, open: Punct, open_loc: Loc) -> Loc {
        if let Some(l) = self.eat(close) {
            return l;
        }
        // A missing closer at a token the parser already stopped at is
        // reported there (clang's `BalancedDelimiterTracker::diagnoseMissingClose`
        // reports at the current token).
        let at = self.loc();
        let note = self.s.diag(DiagId::note_matching, open_loc).arg(quote(open));
        let d = self.s.diag(DiagId::parse_expected, at).arg(quote(close)).with_note(note);
        self.s.emit(d);
        // Unless already at a closing bracket, skip to the closer and
        // take it (clang's `diagnoseMissingClose`: `SkipUntil(close,
        // StopAtSemi | StopBeforeMatch)`).
        if !matches!(
            Self::punct_of(self.tok()),
            Some(Punct::RParen | Punct::RBrace | Punct::RSquare)
        ) && self.skip_until(&[close], true, false)
            && let Some(l) = self.eat(close)
        {
            return l;
        }
        at
    }

    /// Reports `id` at the current token.
    pub(crate) fn error_here(&self, id: DiagId) {
        self.s.error(id, self.loc());
    }

    /// Reports a diagnostic.
    pub(crate) fn emit(&self, d: Diag) {
        self.s.emit(d);
    }

    /// Reports `{what} is not supported yet` at `loc`.
    pub(crate) fn unsupported(&self, what: &str, loc: Loc) {
        self.s.unsupported(what, loc);
    }

    // -- recovery ---------------------------------------------------------------

    /// clang's `SkipUntil`: skips tokens until one of `stops` (consumed
    /// when `consume`), balancing brackets. Stops before `;` when
    /// `stop_at_semi`, and before a closing bracket that closes one the
    /// parser opened earlier. Returns true if a stop token was found.
    pub(crate) fn skip_until(
        &mut self,
        stops: &[Punct],
        stop_at_semi: bool,
        consume: bool,
    ) -> bool {
        let mut first = true;
        loop {
            let t = self.tok();
            if t.kind == TokKind::Eof {
                return false;
            }
            if let Some(p) = Self::punct_of(t)
                && stops.contains(&p)
            {
                if consume {
                    self.bump();
                }
                return true;
            }
            match Self::punct_of(t) {
                Some(Punct::LParen) => {
                    self.bump();
                    self.skip_until(&[Punct::RParen], false, true);
                }
                Some(Punct::LSquare) => {
                    self.bump();
                    self.skip_until(&[Punct::RSquare], false, true);
                }
                Some(Punct::LBrace) => {
                    self.bump();
                    self.skip_until(&[Punct::RBrace], false, true);
                }
                Some(p @ (Punct::RParen | Punct::RSquare | Punct::RBrace)) => {
                    let b = match p {
                        Punct::RParen => Bracket::Paren,
                        Punct::RSquare => Bracket::Square,
                        _ => Bracket::Brace,
                    };
                    if self.depth[b as usize] > 0 && !first {
                        return false;
                    }
                    self.bump();
                }
                Some(Punct::Semi) if stop_at_semi => return false,
                _ => {
                    self.bump();
                }
            }
            first = false;
        }
    }

    /// Skips to the end of the current declaration or statement: up to and
    /// including the next `;` at this nesting level, or up to (not
    /// including) a `}` that closes an enclosing brace.
    pub(crate) fn skip_to_semi(&mut self) {
        self.skip_until(&[Punct::RBrace], true, false);
        if self.at(Punct::RBrace) && self.depth[Bracket::Brace as usize] == 0 {
            // A stray `}` at file scope is reported by the caller.
            return;
        }
        self.eat(Punct::Semi);
    }

    // -- names --------------------------------------------------------------------

    /// Classifies the (possibly qualified) name starting at token index
    /// `i`, without consuming or diagnosing: returns the index after it
    /// and the classification of its last component. `None` if there is
    /// no name at `i` or a qualifier does not name a scope.
    ///
    /// A template-id `X<args>` ([temp.names]) is part of the name: its
    /// arguments are skipped ([`Parser::skip_template_args`]) and a
    /// class or alias template specialization classifies as
    /// `NameClass::Type` (of the template decl, with a dependent
    /// placeholder type: lookahead only asks whether it is a type). If
    /// the list closes in the middle of a `>>`-like token, the returned
    /// index is that token's.
    pub(crate) fn scan_name(&self, i: usize) -> Option<(usize, NameClass, Symbol)> {
        self.scan_name_at(i).map(|(end, _, c, n)| (end, c, n))
    }

    /// [`Parser::scan_name`], also saying whether the name's last
    /// argument list closed on the first `>` of the `>>`-like token at the
    /// returned index.
    pub(crate) fn scan_name_at(&self, i: usize) -> Option<(usize, bool, NameClass, Symbol)> {
        self.scan_name_qual_at(i).map(|(end, split, c, n, _)| (end, split, c, n))
    }

    /// [`Parser::scan_name_at`], also giving the context the name's
    /// qualifier names (`None` when unqualified).
    pub(crate) fn scan_name_qual_at(
        &self,
        i: usize,
    ) -> Option<(usize, bool, NameClass, Symbol, Option<DeclId>)> {
        self.scan_name_walk(i).ok()
    }

    /// Whether the name at token `i` is qualified by a component that
    /// names no scope (an undeclared name, a class template without
    /// arguments, a variable or function before `::`): parsing the
    /// qualifier diagnoses it ("use of undeclared identifier", "requires
    /// template arguments"; clang's `ActOnCXXNestedNameSpecifier`).
    /// Returns the index after the name's last component (before any
    /// template arguments of it), found by tokens alone.
    pub(crate) fn not_a_scope_name(&self, i: usize) -> Option<usize> {
        if !matches!(self.scan_name_walk(i), Err(ScanStop::NotAScope)) {
            return None;
        }
        let mut i = i;
        if Self::punct_of(self.tok_at(i)) == Some(Punct::ColonColon) {
            i += 1;
        }
        loop {
            if self.kw_of(self.tok_at(i)) == Some(Kw::Template) {
                i += 1;
            }
            self.ident_of(self.tok_at(i))?;
            let mut j = i + 1;
            if Self::punct_of(self.tok_at(j)) == Some(Punct::Less) {
                match self.skip_template_args(j) {
                    Some((close, false)) => j = close + 1,
                    _ => return None,
                }
            }
            if Self::punct_of(self.tok_at(j)) != Some(Punct::ColonColon) {
                return Some(i + 1);
            }
            i = j + 1;
        }
    }

    /// [`Parser::scan_name_qual_at`], saying why there is no name.
    ///
    /// Memoized per token index like [`Parser::skip_template_args`]
    /// (spec/parse.md §2.5): each component of a nested template-id is
    /// scanned by several lookahead predicates before it is parsed. Not
    /// while a decltype qualifier is pending (the walk reads it) or after
    /// the stack guard halted the parse.
    fn scan_name_walk(&self, i: usize) -> Result<ScannedName, ScanStop> {
        let memoizable = || self.decltype_scope.is_none() && self.halt.get().is_none();
        if !memoizable() {
            return self.scan_name_walk_in(i, None);
        }
        let stamp = self.s.lookup_stamp();
        {
            let mut m = self.targs_memo.borrow_mut();
            m.refresh(stamp);
            if let Some(&r) = m.names.get(&i) {
                return r;
            }
        }
        let r = self.scan_name_walk_in(i, None);
        let mut m = self.targs_memo.borrow_mut();
        if m.stamp == stamp && memoizable() {
            if m.names.len() >= NAME_MEMO_MAX {
                m.names.clear();
            }
            let _ = m.names.insert(i, r);
        }
        r
    }

    /// [`Parser::scan_name_walk`] from token `i`, the first component
    /// after a `::` whose scope `scope` is already known.
    fn scan_name_walk_in(
        &self,
        mut i: usize,
        scope: Option<DeclId>,
    ) -> Result<ScannedName, ScanStop> {
        use ScanStop::NoName;
        if !self.stack_ok() {
            return Err(NoName);
        }
        let mut qual: Option<DeclId> = scope;
        // A qualifier component was a class template named with dependent
        // arguments outside its own scope ([temp.dep.type]/6): a type
        // found in it by lookahead is a member of an unknown specialization.
        let mut dep_tid = false;
        // The qualifier component was `X<args>::` naming the current
        // instantiation ([temp.dep.type]/1): its members stay dependent.
        let mut qual_cur_inst = false;
        if scope.is_some() {
        } else if let Some(d) = self.decltype_scope_at(i) {
            let q = d.qual.ok_or(NoName)?.0;
            // A parsed-ahead `X<T>::` that is not the current
            // instantiation stays the class template (a dependent marker).
            dep_tid = self.s.ast.decls.get(q).kind() == DeclKind::ClassTemplate;
            qual = Some(q);
            i += 1;
        } else if self.tok_at(i).kind == TokKind::Punct(Punct::ColonColon) {
            qual = Some(self.s.ast.tu);
            i += 1;
        }
        // A qualifier component was a non-dependent class (`O<int>::`):
        // a member class found below it is not dependent, even where the
        // lookahead finds it in the template's pattern because `O<int>`
        // is not instantiated yet (WP-276, WP-1084).
        let mut nondep_class = qual.is_some_and(|q| self.s.is_nondependent_class_qualifier(q));
        loop {
            let template_kw = qual.is_some() && self.kw_of(self.tok_at(i)) == Some(Kw::Template);
            if template_kw {
                i += 1;
            }
            let name = self.ident_of(self.tok_at(i)).ok_or(NoName)?;
            // A name before `::` is looked up for a scope only
            // ([basic.lookup.qual]/1).
            let mut class = if self.tok_at(i + 1).kind == TokKind::Punct(Punct::ColonColon) {
                self.s.classify_nested_name(name, qual)
            } else {
                self.s.classify_name(name, qual)
            };
            if dep_tid && matches!(class, NameClass::Type(..)) {
                class = NameClass::Dependent;
            }
            // A base class specialization's injected-class-name not
            // followed by `<` is a type-name ([temp.local]/1).
            if !template_kw
                && matches!(class, NameClass::TypeTemplate(_))
                && self.tok_at(i + 1).kind != TokKind::Punct(Punct::Less)
                && let Some(c) = self.s.injected_specialization_name(name, qual)
            {
                class = c;
            }
            let mut end = i + 1;
            let mut cur_inst = false;
            if (template_kw || is_template_class(class))
                && self.tok_at(end).kind == TokKind::Punct(Punct::Less)
            {
                let (close, split) = self.skip_template_args(end).ok_or(NoName)?;
                if let NameClass::TypeTemplate(d) = class
                    && !split
                    && self.tok_at(close + 1).kind == TokKind::Punct(Punct::ColonColon)
                    && self.s.ast.decls.get(d).kind() == DeclKind::ClassTemplate
                    && self.template_args_dependent(end, close)
                {
                    if self.s.may_name_current_instantiation(d) {
                        cur_inst = true;
                    } else {
                        dep_tid = true;
                    }
                }
                if let NameClass::TypeTemplate(d) = class
                    && !split
                    && self.tok_at(close + 1).kind == TokKind::Punct(Punct::ColonColon)
                    && matches!(
                        self.s.ast.decls.get(d).kind(),
                        DeclKind::ClassTemplate | DeclKind::TypeAliasTemplate
                    )
                    && !self.template_args_dependent(end, close)
                {
                    nondep_class = true;
                }
                class = type_if_args(class);
                if split {
                    return Ok((close, true, class, name, qual));
                }
                end = close + 1;
            }
            if self.tok_at(end).kind == TokKind::Punct(Punct::ColonColon) {
                // `X<args>::m::` (or `D::m::`, `D` a typedef of `X<args>`)
                // with `m` a member typedef of `X`'s pattern naming
                // `Y<...>`: scan on in `Y` (WP-668). Not for a qualifier
                // in a dependent context (`X<T>::`), nor for a dependent
                // `m` of the current instantiation (`X<S>::` in `X`'s
                // definition, or its injected-class-name `X::`), which
                // names a member of an unknown specialization: there
                // `X<S>::m::name` needs `typename` (WP-676).
                let tid_member = |d: DeclId| {
                    let q = qual?;
                    let template = self.s.ast.decls.get(q).kind() == DeclKind::ClassTemplate;
                    if !template && self.s.is_dependent_ctx(q) {
                        return None;
                    }
                    if (qual_cur_inst || !template)
                        && self.s.ast.decls.get(d).ty().is_some_and(|t| self.s.is_dependent_type(t))
                    {
                        return None;
                    }
                    self.s.member_typedef_class_template(d)
                };
                qual = Some(match class {
                    NameClass::Type(_, d) if let Some(t) = tid_member(d) => t,
                    NameClass::Namespace(d) | NameClass::Type(_, d) => d,
                    // `T::x::`: still dependent.
                    NameClass::Dependent => qual.ok_or(NoName)?,
                    // The injected-class-name `X::` in (a member
                    // declarator of) `X<A...>` ([temp.local]/1).
                    // A class template without arguments names no scope
                    // outside its own ("requires template arguments").
                    NameClass::TypeTemplate(d) if end == i + 1 => {
                        self.s.injected_class_decl(d).ok_or(ScanStop::NotAScope)?
                    }
                    _ => return Err(ScanStop::NotAScope),
                });
                qual_cur_inst = cur_inst;
                nondep_class |= qual.is_some_and(|q| self.s.is_nondependent_class_qualifier(q));
                i = end + 1;
                continue;
            }
            // A type found in a class that is dependent but not the
            // current instantiation here (`L::N` for a local class `L` of
            // a function template, outside `L`; `A::N` for a member class
            // `A` of a class template, outside `A`) is a member of an
            // unknown specialization ([temp.dep.type]/6): without
            // `typename` it is a type only where [temp.res.general]/4
            // makes it one, so `L::N()` and `sizeof(L::N)` are
            // expressions (clang's `computeDeclContext` has no context
            // for `L::`; spec/sema-templates.md §3.1.1, WP-1084).
            if matches!(class, NameClass::Type(..))
                && !nondep_class
                && qual.is_some_and(|q| self.s.is_unentered_dependent_qualifier(q))
            {
                class = NameClass::Dependent;
            }
            return Ok((end, false, class, name, qual));
        }
    }

    /// Lookahead for a `decltype(e)::name` that was not parsed ahead
    /// (WP-843): when `e`, the tokens between the `decltype (` at `kw`
    /// and the `)` before the `::` at `cc`, is an unparenthesized
    /// id-expression naming a variable, data member or enumerator whose
    /// declared type is a non-dependent class or enumeration that is
    /// already defined ([dcl.type.decltype]/1.3: `decltype` gives that
    /// type), the class of `name` looked up in it, with the index after
    /// the qualified name. `None` when the scope needs the expression
    /// parsed (anything else), so the caller falls back to the tokens.
    pub(crate) fn peek_decltype_scope_name(
        &self,
        kw: usize,
        cc: usize,
    ) -> Option<(usize, NameClass)> {
        let (end, class, _) = self.scan_name(kw + 2)?;
        if end + 1 != cc || self.tok_at(end).kind != TokKind::Punct(Punct::RParen) {
            return None;
        }
        let NameClass::Value(d) = class else { return None };
        let decl = self.s.ast.decls.get(d);
        if !matches!(
            decl.kind(),
            DeclKind::Var | DeclKind::ParmVar | DeclKind::Field | DeclKind::EnumConstant
        ) {
            return None;
        }
        let ty = decl.ty()?;
        let t = &self.s.ast.types;
        if t.is_dependent(ty) || t.is_reference(ty) {
            return None;
        }
        let r = t.as_record(ty).or_else(|| t.as_enum(ty))?;
        let def = self.s.ast.definition(r)?;
        if self.s.is_dependent_ctx(def) {
            return None;
        }
        let (end, split, class, _, _) = self.scan_name_walk_in(cc + 1, Some(def)).ok()?;
        (!split).then_some((end, class))
    }

    /// Whether the name at token `i` is qualified by a non-dependent
    /// class or namespace in which lookup finds a non-type (a variable,
    /// function or enumerator).
    pub(crate) fn nontype_scope_name(&self, i: usize) -> bool {
        matches!(
            self.scan_name_qual_at(i),
            Some((end, false, NameClass::Value(_), _, Some(q)))
                if end > i + 1 && !self.s.is_dependent_ctx(q)
        )
    }

    /// Whether the name at token `i` is qualified by a non-dependent
    /// class or namespace in which lookup finds nothing (clang's
    /// `TryAnnotateName` giving `ANK_Unresolved` after a scope).
    pub(crate) fn unresolved_scope_name(&self, i: usize) -> bool {
        matches!(
            self.scan_name_qual_at(i),
            Some((end, false, NameClass::Undeclared, _, Some(q)))
                if end > i + 1 && !self.s.is_dependent_ctx(q)
        )
    }

    /// Whether the name at token `i` is qualified by a class or
    /// enumeration that is incomplete as a nested-name-specifier
    /// ([`Sema::nns_ctx_incomplete`]): the parse of the qualified name
    /// reports it, wherever it stands.
    ///
    /// [`Sema::nns_ctx_incomplete`]: kiln_sema::Sema::nns_ctx_incomplete
    pub(crate) fn incomplete_scope_name(&self, i: usize) -> bool {
        matches!(
            self.scan_name_qual_at(i),
            Some((end, false, NameClass::Undeclared, _, Some(q)))
                if end > i + 1 && self.s.nns_ctx_incomplete(q)
        )
    }

    /// Whether the template argument list between `<` at `lt` and its
    /// closer at `close` names a template parameter or a dependent name
    /// ([temp.dep.type]/7): lookahead approximation by the unqualified
    /// identifiers in it.
    fn template_args_dependent(&self, lt: usize, close: usize) -> bool {
        (lt + 1..close).any(|j| {
            let Some(name) = self.ident_of(self.tok_at(j)) else { return false };
            if matches!(
                Self::punct_of(self.tok_at(j - 1)),
                Some(Punct::ColonColon | Punct::Period | Punct::Arrow)
            ) {
                return false;
            }
            let kind = |d: DeclId| self.s.ast.decls.get(d).kind();
            let dep_ty = |d: DeclId| {
                self.s.ast.decls.get(d).ty().is_some_and(|t| self.s.is_dependent_type(t))
            };
            match self.s.classify_name(name, None) {
                NameClass::Dependent => true,
                NameClass::Type(t, d) => {
                    matches!(kind(d), DeclKind::TemplateTypeParm | DeclKind::TemplateTemplateParm)
                        || self.s.is_dependent_type(t)
                }
                NameClass::Value(d) => kind(d) == DeclKind::NonTypeTemplateParm || dep_ty(d),
                NameClass::TypeTemplate(d) => kind(d) == DeclKind::TemplateTemplateParm,
                _ => false,
            }
        })
    }

    /// Skips the template argument list whose `<` is at index `i`
    /// ([temp.names]/4): a `<` opens a nested list only after a name that
    /// classifies as a template (or after `template`); `>` (or each `>` of
    /// a `>>`-like token) closes the innermost list when no bracket is
    /// open inside it. Returns the index of the token that closes the
    /// list and whether it closed before that token's last character (a
    /// `>>` with one list open, `>>=`). A `>=` is a relational operator
    /// inside the list, never a closer. `None` if a bracket closes
    /// unbalanced, or a `;` or the end of input is reached first.
    ///
    /// Memoized per `<` index (spec/parse.md §2.5): nested lists are
    /// scanned by the enclosing list's scan, and each level of
    /// `A<B<C<...>>>` asks again when it is parsed, so without the memo
    /// the parse is quadratic in the nesting depth.
    pub(crate) fn skip_template_args(&self, i: usize) -> Option<(usize, bool)> {
        let stamp = self.s.lookup_stamp();
        if let Some(&r) = self.targs_memo.borrow_mut().valid_for(stamp).get(&i) {
            return r;
        }
        let r = self.scan_template_args(i);
        let mut m = self.targs_memo.borrow_mut();
        // A nested scan may have refreshed the memo; keep only results
        // computed under the current stamp.
        if m.stamp == stamp {
            let _ = m.map.insert(i, r);
        }
        r
    }

    /// Clears the template-argument memo: the token indices or the
    /// tokens themselves changed.
    fn forget_scans(&mut self) {
        let m = self.targs_memo.get_mut();
        m.map.clear();
        m.names.clear();
    }

    /// The token after splice `[: r :]` at `j` inside a template argument
    /// scan; the `<` after it opens a nested list (pushed on `stack`)
    /// when `template` (`templ_kw`) or `typename` precedes the splice.
    fn scan_splice_in_args(
        &self,
        j: usize,
        templ_kw: bool,
        stack: &mut Vec<Punct>,
    ) -> Option<usize> {
        let after = self.splice_end(j)?;
        let typename = self.kw_of(self.tok_at(j - 1)) == Some(Kw::Typename);
        if (templ_kw || typename) && self.tok_at(after).kind == TokKind::Punct(Punct::Less) {
            stack.push(Punct::Less);
            return Some(after + 1);
        }
        Some(after)
    }

    /// [`Parser::skip_template_args`] without the memo.
    #[allow(clippy::too_many_lines, reason = "one token-scanning loop, an arm per token kind")]
    fn scan_template_args(&self, i: usize) -> Option<(usize, bool)> {
        let mut stack = vec![Punct::Less];
        let mut j = i + 1;
        let mut after_template_kw = false;
        loop {
            let t = self.tok_at(j);
            let templ_kw = after_template_kw;
            after_template_kw = false;
            match t.kind {
                TokKind::Eof => return None,
                // A splice `[: r :]` is skipped whole (`>` is an operator
                // inside it); a `<` after it opens a list after
                // `template` or `typename` (P2996 [temp.names]/3).
                TokKind::Punct(Punct::SpliceOpen) => {
                    j = self.scan_splice_in_args(j, templ_kw, &mut stack)?;
                    continue;
                }
                TokKind::Identifier(_) if self.kw_of(t) == Some(Kw::Template) => {
                    after_template_kw = true;
                    j += 1;
                    continue;
                }
                TokKind::Identifier(_) if self.kw_of(t) == Some(Kw::Decltype) => {
                    // `decltype(e)::name<...>` (WP-119): the components'
                    // own lists are skipped with the name.
                    j = match self.decltype_scope_end(j) {
                        Some(cc) => self.skip_qualified_rest(cc).unwrap_or(cc + 1),
                        None => j + 1,
                    };
                    continue;
                }
                TokKind::Identifier(_)
                    if matches!(
                        self.kw_of(t),
                        Some(
                            Kw::StaticCast | Kw::DynamicCast | Kw::ReinterpretCast | Kw::ConstCast
                        )
                    ) && self.tok_at(j + 1).kind == TokKind::Punct(Punct::Less) =>
                {
                    // `static_cast<T>(e)` ([expr.post.general]): the cast's
                    // `<` always opens a type list of its own.
                    stack.push(Punct::Less);
                    j += 2;
                    continue;
                }
                TokKind::Identifier(_) if self.kw_of(t).is_some() => {
                    j += 1;
                    continue;
                }
                TokKind::Identifier(_) if templ_kw => {
                    // `template name<`: always a template-id.
                    if self.tok_at(j + 1).kind == TokKind::Punct(Punct::Less) {
                        stack.push(Punct::Less);
                        j += 2;
                    } else {
                        j += 1;
                    }
                    continue;
                }
                TokKind::Identifier(_) | TokKind::Punct(Punct::ColonColon) => {
                    if let Some((end, split, _, _)) = self.scan_name_at(j)
                        && end > j
                    {
                        if !split {
                            j = end;
                            continue;
                        }
                        // The name's own list took the first `>` of
                        // token `end`; the rest may close this one.
                        if let Some(r) =
                            close_angles(&mut stack, Self::punct_of(self.tok_at(end)), 1)
                        {
                            return Some((end, r));
                        }
                        j = end + 1;
                        continue;
                    }
                    // A member pointer's qualifier `X<T>::*` ([dcl.mptr];
                    // boost's `safe_bool<I base<I, T>::*>`) is no name on
                    // its own, but its components' lists are still nested
                    // lists: skipped up to the `*`.
                    if let Some(star) = self.member_pointer_at(j) {
                        j = star;
                        continue;
                    }
                    j += 1;
                    continue;
                }
                _ => {}
            }
            let Some(p) = Self::punct_of(t) else {
                j += 1;
                continue;
            };
            match p {
                Punct::LParen | Punct::LSquare | Punct::LBrace => stack.push(p),
                Punct::RParen | Punct::RSquare | Punct::RBrace => {
                    let want = match p {
                        Punct::RParen => Punct::LParen,
                        Punct::RSquare => Punct::LSquare,
                        _ => Punct::LBrace,
                    };
                    if stack.last() != Some(&want) {
                        return None;
                    }
                    stack.pop();
                }
                // A `;` ends the scan unless inside braces (a
                // requires-expression or lambda body in the argument).
                Punct::Semi if !stack.contains(&Punct::LBrace) => return None,
                // `>=` is a relational operator inside a template
                // argument ([temp.names]/4 splits only `>` and `>>`;
                // clang parses `B<n >= 1>` as one argument).
                Punct::GreaterEqual => {}
                _ => {
                    if let Some(r) = close_angles(&mut stack, Some(p), 0) {
                        return Some((j, r));
                    }
                }
            }
            j += 1;
        }
    }

    /// Consumes the first `>` of the current token (`>`, `>>`, `>=`,
    /// `>>=`) as the end of a template argument list ([temp.names]/4):
    /// the rest stays the current token. Returns the `>`'s location.
    pub(crate) fn split_greater(&mut self) -> Loc {
        let t = self.tok();
        let rest = match Self::punct_of(t) {
            Some(Punct::GreaterGreater) => Punct::Greater,
            Some(Punct::GreaterEqual) => Punct::Equal,
            Some(Punct::GreaterGreaterEqual) => Punct::GreaterEqual,
            _ => return self.bump().span.lo,
        };
        let lo = t.span.lo;
        let mid = lo.with_offset(1);
        self.s.note_split_greater(lo);
        if rest == Punct::Greater {
            self.s.note_split_greater(mid);
        }
        self.forget_scans();
        self.cur = Token {
            kind: TokKind::Punct(rest),
            span: Span::new(mid, t.span.hi),
            flags: TokFlags::empty(),
        };
        self.prev_hi = mid;
        self.prev_lo = lo;
        lo
    }

    /// The index of the `::` when a `decltype ( expression ) ::`, a
    /// splice-scope-specifier `[: r :] ::` or a pack-index-specifier
    /// `T ... [ expression ] ::` starts at token `i`
    /// ([expr.prim.id.qual]: a *computed-type-specifier* as the first
    /// component of a nested-name-specifier; [dcl.type.pack.index]).
    /// `decltype(auto)` is never one.
    pub(crate) fn decltype_scope_end(&self, i: usize) -> Option<usize> {
        let t = self.tok_at(i);
        match self.kw_of(t) {
            // A splice-scope-specifier `template [: r :] < args > ::`
            // ([expr.prim.id.qual]/1, P2996): `template` is required
            // before the arguments (as GCC 16.1).
            Some(Kw::Template) => {
                let after = self.splice_end(i + 1)?;
                if self.tok_at(after).kind != TokKind::Punct(Punct::Less) {
                    return None;
                }
                let (end, split) = self.skip_template_args(after)?;
                (!split && self.tok_at(end + 1).kind == TokKind::Punct(Punct::ColonColon))
                    .then_some(end + 1)
            }
            Some(Kw::Decltype) => {
                if self.tok_at(i + 1).kind != TokKind::Punct(Punct::LParen)
                    || (self.kw_of(self.tok_at(i + 2)) == Some(Kw::Auto)
                        && self.tok_at(i + 3).kind == TokKind::Punct(Punct::RParen))
                {
                    return None;
                }
                let after = self.skip_balanced(i + 1)?;
                (self.tok_at(after).kind == TokKind::Punct(Punct::ColonColon)).then_some(after)
            }
            Some(_) => None,
            None if matches!(t.kind, TokKind::Identifier(_)) => self.pack_index_scope_end(i),
            // A splice-scope-specifier `[: r :] ::` ([expr.prim.id.qual]/1).
            None => {
                let after = self.splice_end(i)?;
                (self.tok_at(after).kind == TokKind::Punct(Punct::ColonColon)).then_some(after)
            }
        }
    }

    /// [`Parser::decltype_scope_end`] for `T ... [ expression ] ::` with
    /// `T` a type name (C++26 [dcl.type.pack.index]).
    fn pack_index_scope_end(&self, i: usize) -> Option<usize> {
        if self.tok_at(i + 1).kind != TokKind::Punct(Punct::Ellipsis)
            || self.tok_at(i + 2).kind != TokKind::Punct(Punct::LSquare)
            || !matches!(self.scan_name(i), Some((e, NameClass::Type(..), _)) if e == i + 1)
        {
            return None;
        }
        let after = self.skip_balanced(i + 2)?;
        (self.tok_at(after).kind == TokKind::Punct(Punct::ColonColon)).then_some(after)
    }

    /// The index after the bracket group whose opener is token `i`
    /// (`None` at an unbalanced closer or the end of input).
    pub(crate) fn skip_balanced(&self, mut i: usize) -> Option<usize> {
        let mut stack: Vec<Punct> = Vec::new();
        loop {
            match self.tok_at(i).kind {
                TokKind::Eof => return None,
                TokKind::Punct(Punct::LParen) => stack.push(Punct::RParen),
                TokKind::Punct(Punct::LSquare) => stack.push(Punct::RSquare),
                TokKind::Punct(Punct::LBrace) => stack.push(Punct::RBrace),
                // A matching closer is popped by the guard.
                TokKind::Punct(c @ (Punct::RParen | Punct::RSquare | Punct::RBrace))
                    if stack.pop() != Some(c) =>
                {
                    return None;
                }
                _ => {}
            }
            i += 1;
            if stack.is_empty() {
                return Some(i);
            }
        }
    }

    /// At a `decltype ( expression ) ::` ([`Parser::decltype_scope_end`]),
    /// parses the decltype-specifier and records it as the pending first
    /// component of the nested-name-specifier at the `::`, diagnosing a
    /// type that is not a class or enumeration. True if the current token
    /// is then that `::` (whose annotation may carry an error).
    pub(crate) fn annotate_decltype_scope(&mut self) -> bool {
        if self.decltype_scope_at(self.pos).is_some() {
            return true;
        }
        let loc = self.loc();
        if self.kw() == Some(Kw::Template) && self.peek_is(1, Punct::SpliceOpen) {
            // `template [: r :] < args > ::`: the specialization is the scope.
            self.bump();
            let open = self.loc();
            let (e, _) = self.parse_splice_specifier().unzip();
            let ty = match e {
                Some(e) if self.at(Punct::Less) => self
                    .parse_template_arg_list()
                    .and_then(|(args, _, r)| self.s.act_on_splice_template_type(e, &args, open, r)),
                _ => None,
            };
            if !self.at(Punct::ColonColon) {
                return false;
            }
            let qual = ty.and_then(|t| self.s.act_on_nested_name_decltype(t, loc));
            self.decltype_scope =
                Some(DecltypeScope { level: self.replay.len(), pos: self.pos, qual, loc });
            return true;
        }
        if self.at_splice() {
            let e = self.parse_splice_operand();
            if !self.at(Punct::ColonColon) {
                return false;
            }
            let qual = e.and_then(|e| self.s.act_on_nested_name_splice(e, loc));
            self.decltype_scope =
                Some(DecltypeScope { level: self.replay.len(), pos: self.pos, qual, loc });
            return true;
        }
        let ty = if self.ident().is_some() {
            self.parse_pack_index_scope(loc)
        } else {
            self.parse_decltype_specifier().1
        };
        if !self.at(Punct::ColonColon) {
            return false;
        }
        let qual = ty.and_then(|t| self.s.act_on_nested_name_decltype(t, loc));
        self.decltype_scope =
            Some(DecltypeScope { level: self.replay.len(), pos: self.pos, qual, loc });
        true
    }

    /// At a `T ... [ expression ] ::` ([`Parser::pack_index_scope_end`]),
    /// parses the pack-index-specifier up to the `::`. `None` after an
    /// error.
    fn parse_pack_index_scope(&mut self, loc: Loc) -> Option<QualType> {
        let Some((_, NameClass::Type(ty, d), name)) = self.scan_name(self.pos) else {
            return None;
        };
        self.bump();
        self.s.mark_type_referenced(d);
        self.s.check_type_name_access(name, None, d, loc);
        match self.parse_pack_index(loc) {
            Ok(Some((ellipsis, index, _))) => {
                let ty = self.s.act_pack_indexing_type_named(ty, name, loc, ellipsis, index);
                (!self.s.is_error_type(ty)).then_some(ty)
            }
            _ => None,
        }
    }

    /// The index of the last `::` of the qualified name at token `i`
    /// when its qualifier is a class template specialization `X<A...>`
    /// in which the lookahead does not find the name: the lookahead
    /// looks in `X`'s primary pattern only, and the name may be declared
    /// by a partial or explicit specialization, or inherited from a base
    /// the specialization's instantiation names. Such a qualifier is
    /// parsed ahead ([`Parser::annotate_template_scope`]), as clang
    /// annotates a scope before disambiguating.
    pub(crate) fn template_scope_end(&self, i: usize) -> Option<usize> {
        if self.decltype_scope_at(i).is_some() || self.decltype_scope_end(i).is_some() {
            return None;
        }
        let (end, split, class, _, qual) = self.scan_name_qual_at(i)?;
        let q = qual?;
        if split
            || !matches!(class, NameClass::Dependent | NameClass::Undeclared)
            || end < i + 3
            || !self.unresolved_specialization_scope(q)
            || self.tok_at(end - 2).kind != TokKind::Punct(Punct::ColonColon)
        {
            return None;
        }
        Some(end - 2)
    }

    /// True if the lookahead's scope `q` stands for a class template
    /// specialization it cannot look into: a class template (`X<A>::`,
    /// looked up in the primary pattern), or a typedef naming a
    /// specialization that is not instantiated yet (`hours::period` with
    /// `typedef duration<long, ratio<3600>> hours`), or an alias template
    /// whose pattern names no class template it can follow (`A<R>::` with
    /// `template<class T> using A = decltype(f(T()))`), or a member
    /// typedef of a class template's pattern naming a dependent type
    /// (`rr<V>::type::`; WP-724), or a member typedef of a class
    /// template's primary pattern reached through `X<A>::m::` (an explicit
    /// or partial specialization may declare `m` otherwise: folly's
    /// `clock_traits<steady_clock>::spec::`; WP-726).
    fn unresolved_specialization_scope(&self, q: DeclId) -> bool {
        use kiln_ast::decl::DeclKind;
        let ast = &self.s.ast;
        match ast.decls.get(q).kind() {
            DeclKind::ClassTemplate | DeclKind::TypeAliasTemplate => true,
            DeclKind::Typedef | DeclKind::TypeAlias => {
                let ty = ast.decls.get(q).ty().unwrap_or(QualType::ERROR);
                ast.types.as_record(ty).is_some_and(|r| {
                    ast.definition(r).is_none() && ast.spec_info(ast.first(r)).is_some()
                }) || self.s.is_dependent_type(ty) && self.in_class_template_pattern(q)
                    || ast
                        .decls
                        .get(q)
                        .ctx()
                        .and_then(|c| ast.described_template(ast.first(c)))
                        .is_some_and(|t| !self.s.may_name_current_instantiation(t))
            }
            _ => false,
        }
    }

    /// Whether `d` is a member of a class template's pattern: a member
    /// typedef naming a template parameter (`rr<V>::type::` with
    /// `typedef T type;` in `rr`'s pattern) that the lookahead reached
    /// through a specialization it does not instantiate (WP-724).
    fn in_class_template_pattern(&self, d: DeclId) -> bool {
        use kiln_ast::decl::DeclKind;
        let ast = &self.s.ast;
        ast.decls
            .get(d)
            .ctx()
            .and_then(|c| ast.described_template(c))
            .is_some_and(|t| ast.decls.get(t).kind() == DeclKind::ClassTemplate)
    }

    /// At a qualified name whose qualifier the lookahead cannot resolve
    /// ([`Parser::template_scope_end`]): parses the qualifier up to (not
    /// including) its last `::` and records it as that `::`'s pending
    /// scope, like a `decltype(e)::` ([`Parser::annotate_decltype_scope`]).
    /// True if the current token is then that `::`.
    pub(crate) fn annotate_template_scope(&mut self) -> bool {
        let Some(cc) = self.template_scope_end(self.pos) else { return false };
        let loc = self.loc();
        let r = self.parse_nested_name_upto(Some(cc));
        if self.pos != cc || !self.at(Punct::ColonColon) {
            return false;
        }
        let qual = r.ok().flatten().map(|q| (q.ctx, q.nns));
        self.decltype_scope =
            Some(DecltypeScope { level: self.replay.len(), pos: self.pos, qual, loc });
        true
    }

    /// The pending decltype qualifier whose `::` is token `i`.
    fn decltype_scope_at(&self, i: usize) -> Option<DecltypeScope> {
        self.decltype_scope.filter(|d| d.level == self.replay.len() && d.pos == i)
    }

    /// At the `::` of a pending decltype qualifier at token `i`: the
    /// scope it names (`Err` after an error).
    pub(crate) fn decltype_scope_qual_at(&self, i: usize) -> Option<Result<DeclId, ()>> {
        self.decltype_scope_at(i).map(|d| d.qual.map(|q| q.0).ok_or(()))
    }

    /// After a nested-name-specifier that failed just past one of its
    /// `::`s: skips the components that follow it
    /// ([`Parser::skip_qualified_rest`]).
    pub(crate) fn skip_after_failed_qualifier(&mut self) {
        let Some(cc) = self.pos.checked_sub(1) else { return };
        if self.tok_at(cc).kind != TokKind::Punct(Punct::ColonColon) {
            return;
        }
        if let Some(end) = self.skip_qualified_rest(cc) {
            while self.pos < end {
                self.bump();
            }
        }
    }

    /// The index after the components following the `::` at token `cc`,
    /// skipped without classifying them (the scope is unknown): `template`,
    /// identifiers, template argument lists after a `<` that close, and
    /// `::`s (not a `::*`). `None` if no identifier follows a `::`.
    pub(crate) fn skip_qualified_rest(&self, cc: usize) -> Option<usize> {
        let mut i = cc + 1;
        loop {
            if self.kw_of(self.tok_at(i)) == Some(Kw::Template) {
                i += 1;
            }
            self.ident_of(self.tok_at(i))?;
            i += 1;
            if self.tok_at(i).kind == TokKind::Punct(Punct::Less) {
                // Not an argument list unless it closes (`x < y` is an
                // expression).
                let Some((close, split)) = self.skip_template_args(i) else { return Some(i) };
                if split {
                    return Some(close);
                }
                i = close + 1;
            }
            if self.tok_at(i).kind != TokKind::Punct(Punct::ColonColon)
                || self.tok_at(i + 1).kind == TokKind::Punct(Punct::Star)
            {
                return Some(i);
            }
            i += 1;
        }
    }

    /// Where a name starting at the current token begins: the `decltype`
    /// of a pending decltype qualifier, else the current token.
    pub(crate) fn name_begin(&self) -> Loc {
        self.decltype_scope_at(self.pos).map_or_else(|| self.loc(), |d| d.loc)
    }

    /// True when the current token is the `::` of a pending decltype
    /// qualifier that named a scope (no error).
    pub(crate) fn decltype_scope_ok(&self) -> bool {
        self.decltype_scope_at(self.pos).is_some_and(|d| d.qual.is_some())
    }

    /// Skips the rest of a qualified name after a `decltype(e)` qualifier
    /// that was diagnosed (clang recovers silently): `::`, then
    /// `template`, identifiers, template argument lists and `::`s.
    pub(crate) fn skip_failed_decltype_scope(&mut self) {
        if !self.at(Punct::ColonColon) {
            return;
        }
        self.bump();
        loop {
            if self.kw_of(self.tok()) == Some(Kw::Template) {
                self.bump();
            }
            if self.ident_of(self.tok()).is_none() {
                return;
            }
            self.bump();
            if self.at(Punct::Less)
                && let Some((close, false)) = self.skip_template_args(self.pos)
            {
                while self.pos <= close && !self.at_eof() {
                    self.bump();
                }
            }
            if !self.at(Punct::ColonColon) {
                return;
            }
            self.bump();
        }
    }

    /// True when the current token is the `::` of a pending decltype
    /// qualifier.
    pub(crate) fn at_decltype_scope(&self) -> bool {
        self.is_decltype_scope_at(self.pos)
    }

    /// True when token `i` is the `::` of a pending decltype qualifier.
    pub(crate) fn is_decltype_scope_at(&self, i: usize) -> bool {
        self.decltype_scope_at(i).is_some()
    }

    /// Parses an optional nested-name-specifier (`::`, `N::`, `N::M::`),
    /// diagnosing unknown names. Returns `Ok(None)` when there is none,
    /// `Err(())` after an error.
    pub(crate) fn parse_nested_name(&mut self) -> Result<Option<DeclId>, ()> {
        Ok(self.parse_nested_name_nns()?.map(|q| q.ctx))
    }

    /// [`Parser::parse_nested_name`], also building the specifier as
    /// written (namespace aliases and typedef names kept).
    pub(crate) fn parse_nested_name_nns(&mut self) -> Result<Option<Qualifier>, ()> {
        self.parse_nested_name_upto(None)
    }

    /// [`Parser::parse_nested_name_nns`], stopping before the `::` at
    /// token index `limit` (left unconsumed) when there is one.
    pub(crate) fn parse_nested_name_upto(
        &mut self,
        limit: Option<usize>,
    ) -> Result<Option<Qualifier>, ()> {
        let mut start = self.loc();
        let mut qual: Option<(DeclId, NnsId)> = None;
        if self.decltype_scope_end(self.pos).is_some() {
            self.annotate_decltype_scope();
        }
        if let Some(d) = self.decltype_scope_at(self.pos) {
            self.bump();
            start = d.loc;
            qual = Some(d.qual.ok_or(())?);
        } else if self.at(Punct::ColonColon) {
            self.bump();
            qual = Some((self.s.ast.tu, self.s.nns_global()));
        }
        loop {
            // `nns template name<args>::` ([temp.names]/5).
            let template_kw = qual.is_some()
                && self.kw_of(self.tok_at(self.pos)) == Some(Kw::Template)
                && self.ident_of(self.tok_at(self.pos + 1)).is_some();
            let k = usize::from(template_kw);
            let Some(name) = self.ident_of(self.tok_at(self.pos + k)) else { break };
            let next = self.tok_at(self.pos + k + 1).kind;
            if !template_kw && next == TokKind::Punct(Punct::ColonColon) {
                let stop = limit == Some(self.pos + 1);
                let loc = self.bump().span.lo;
                let cc = self.tok().span.lo;
                if !stop {
                    self.bump();
                }
                self.s.nns_start = Some(start);
                let q = self.s.act_on_nested_name_component(qual, name, loc);
                self.s.nns_start = None;
                let q = q.ok_or(())?;
                // The next name is looked up in `q`, which must be
                // complete; a pointer-to-member's class need not be
                // ([dcl.mptr]/1, clang checks only at lookup).
                if !stop && !self.at(Punct::Star) {
                    self.s.nns_start = Some(start);
                    if !self.s.require_complete_nns_ctx(q.0, loc) {
                        // Recover past the rest of the specifier, as
                        // clang's invalid `CXXScopeSpec` does; the caller
                        // sees the final name.
                        self.skip_nested_name_rest();
                        return Err(());
                    }
                }
                self.s.act_on_nns_component_use(q.1, cc);
                qual = Some(q);
                if stop {
                    break;
                }
                continue;
            }
            if next != TokKind::Punct(Punct::Less) {
                break;
            }
            // `name < args > ::` with the list closing on a whole `>`.
            match self.skip_template_args(self.pos + k + 1) {
                Some((close, false))
                    if self.tok_at(close + 1).kind == TokKind::Punct(Punct::ColonColon) => {}
                _ => break,
            }
            let dep_prefix = qual.is_some_and(|(c, _)| self.s.is_dependent_ctx(c));
            let class = if dep_prefix {
                NameClass::Dependent
            } else {
                self.s.classify_name(name, qual.map(|q| q.0))
            };
            if !template_kw && !is_template_class(class) {
                if class != NameClass::Dependent {
                    break;
                }
                // [temp.names]/3: without `template` the `<` after a
                // dependent name is less-than. It is diagnosed and read as
                // a list only when nothing but a list fits: `<>` or a
                // leading unambiguous type (clang's `isTemplateArgumentList`
                // returning True). `if_c<rank<T>::value < rank<U>::value,
                // U, T>::type` is a comparison.
                let less = self.pos + k + 1;
                let first = self.tok_at(less + 1).kind;
                if !matches!(first, TokKind::Punct(Punct::Greater | Punct::GreaterGreater))
                    && !self.is_cxx_type_id_at(less + 1, TypeIdContext::Unambiguous).0
                {
                    break;
                }
                let text = self.s.ast.symbols().as_str(name).to_owned();
                let d = self
                    .s
                    .diag(
                        DiagId::parse_missing_dependent_template_kw,
                        self.tok_at(self.pos).span.lo,
                    )
                    .arg(text);
                self.emit(d);
            }
            let kw = if template_kw { Some(self.bump().span.lo) } else { None };
            let loc = self.bump().span.lo;
            let Some((args, _, _)) = self.parse_template_arg_list() else { return Err(()) };
            let stop = limit == Some(self.pos);
            if !stop {
                self.expect(Punct::ColonColon);
            }
            self.s.nns_start = Some(start);
            let q = self.nested_name_template_id(class, qual, name, &args, kw, loc);
            self.s.nns_start = None;
            qual = Some(q?);
            if stop {
                break;
            }
        }
        Ok(qual.map(|(ctx, nns)| Qualifier { ctx, nns, loc: start }))
    }

    /// Skips the `name ::` and `name<args> ::` components left after a
    /// nested-name-specifier that failed (diagnosed), so the declarator
    /// goes on at its unqualified-id (clang annotates the whole invalid
    /// `CXXScopeSpec`).
    pub(crate) fn skip_nested_name_rest(&mut self) {
        loop {
            let mut i = self.pos;
            if self.kw_of(self.tok_at(i)) == Some(Kw::Template) {
                i += 1;
            }
            if self.ident_of(self.tok_at(i)).is_none() {
                return;
            }
            let mut j = i + 1;
            if Self::punct_of(self.tok_at(j)) == Some(Punct::Less) {
                match self.skip_template_args(j) {
                    Some((close, false)) => j = close + 1,
                    _ => return,
                }
            }
            if Self::punct_of(self.tok_at(j)) != Some(Punct::ColonColon) {
                return;
            }
            while self.pos <= j {
                self.bump();
            }
        }
    }

    /// The `name<args>::` component of a nested-name-specifier after `qual`,
    /// with the location of its `template` keyword (`kw`) if one is written.
    fn nested_name_template_id(
        &mut self,
        class: NameClass,
        qual: Option<(DeclId, NnsId)>,
        name: Symbol,
        args: &[kiln_ast::types::TemplateArg],
        kw: Option<Loc>,
        loc: Loc,
    ) -> Result<(DeclId, NnsId), ()> {
        let template_kw = kw.is_some();
        // The component begins at its `template` keyword, if any (clang's
        // last-qualifier-name location).
        let head = kw.unwrap_or(loc);
        let dep_prefix = qual.is_some_and(|(c, _)| self.s.is_dependent_ctx(c));
        let prefix_nns = qual.map(|q| q.1);
        Ok(match (class, qual) {
            (NameClass::TypeTemplate(d), _) if !dep_prefix => {
                // [class.access]/4: the template name before `<...>::` is
                // access-checked like any other nested-name component.
                self.s.check_type_name_access(name, qual.map(|q| q.0), d, loc);
                let tn = self.s.template_name_for(d);
                let Ok(kiln_sema::templ::actions::TemplateIdRef::Type(ty)) =
                    self.s.act_template_id(tn, args, loc, loc)
                else {
                    return Err(());
                };
                self.s
                    .act_on_nested_name_template_id(prefix_nns, d, ty, template_kw, head)
                    .ok_or(())?
            }
            (_, Some((c, pn))) if dep_prefix || class == NameClass::Dependent => {
                let list = self.s.ast.types.arg_list(args);
                let ty = self.s.act_dependent_name_at(
                    pn,
                    name,
                    Some(list),
                    kiln_ast::types::TagKeyword::None,
                    loc,
                );
                self.s
                    .act_on_nested_name_template_id(Some(pn), c, ty, template_kw, head)
                    .ok_or(())?
            }
            _ => {
                let text = self.s.ast.symbols().as_str(name).to_owned();
                let d = self.s.diag(DiagId::sema_no_template_named, loc).arg(text);
                self.emit(d);
                return Err(());
            }
        })
    }
}

/// True for a name class that a following `<` makes a template-id.
pub(crate) fn is_template_class(c: NameClass) -> bool {
    matches!(
        c,
        NameClass::TypeTemplate(_)
            | NameClass::VarTemplate(_)
            | NameClass::FunctionTemplate(_)
            | NameClass::Concept(_)
    )
}

/// A scanned name: the index after it, whether its last argument list
/// closed inside a `>>`-like token, its class, last component and
/// qualifier context ([`Parser::scan_name_qual_at`]).
type ScannedName = (usize, bool, NameClass, Symbol, Option<DeclId>);

/// Why [`Parser::scan_name_walk`] found no name.
#[derive(Copy, Clone)]
enum ScanStop {
    /// No name, or one lookahead cannot classify.
    NoName,
    /// A qualifier component names no scope ([`Parser::not_a_scope_name`]).
    NotAScope,
}

/// A class or alias template specialization scans as a type.
fn type_if_args(c: NameClass) -> NameClass {
    match c {
        NameClass::TypeTemplate(d) => NameClass::Type(QualType::DEPENDENT, d),
        c => c,
    }
}

/// Applies the `>` characters of token `p` from character `from` to the
/// open-bracket `stack` of [`Parser::skip_template_args`]: each `>` closes
/// an innermost `<`. `Some(split)` when the stack empties (`split`: before
/// the token's last character).
fn close_angles(stack: &mut Vec<Punct>, p: Option<Punct>, from: usize) -> Option<bool> {
    let chars: &[bool] = match p {
        Some(Punct::Greater) => &[true],
        Some(Punct::GreaterGreater) => &[true, true],
        Some(Punct::GreaterEqual) => &[true, false],
        Some(Punct::GreaterGreaterEqual) => &[true, true, false],
        _ => return None,
    };
    for (k, &is_gt) in chars.iter().enumerate().skip(from) {
        if !is_gt || stack.last() != Some(&Punct::Less) {
            return None;
        }
        stack.pop();
        if stack.is_empty() {
            return Some(k + 1 < chars.len());
        }
    }
    None
}

/// `'p'` as clang quotes a token in "expected" diagnostics.
pub(crate) fn quote(p: Punct) -> String {
    format!("'{}'", p.as_str())
}
