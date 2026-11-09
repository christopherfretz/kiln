//! The flow-sensitive uninitialized-values analysis of a function body
//! (`-Wuninitialized`, `-Wsometimes-uninitialized`,
//! `-Wconditional-uninitialized`, `-Wuninitialized-const-reference`;
//! spec/sema.md §7.14, docs/design/uninitialized-values.md).
//!
//! A port of clang 19's `clang/lib/Analysis/UninitializedValues.cpp`
//! (`isTrackedVar`, `ClassifyRefs`, `TransferFunctions`, `getUninitUse`,
//! `runUninitializedVariablesAnalysis`) and of the reporting in
//! `clang/lib/Sema/AnalysisBasedWarnings.cpp` (`UninitValsDiagReporter`,
//! `DiagnoseUninitializedUse`, `DiagUninitUse`,
//! `SuggestInitializationFixit`).
//!
//! The analysis runs over the body's [`Cfg`] after the unreachable-code
//! scan, only when a body declared a local variable without an
//! initializer or named one inside its own initializer
//! ([`Sema::note_uninit_var`], clang's "no tracked variable" fast path
//! tightened: a tracked variable with an initializer can only be
//! uninitialized through a self-reference, since C++ forbids jumping
//! past its initialization). Values are two bits per variable in dense
//! `u64` words, one row per block; every buffer lives in [`Scratch`] and
//! is reused from body to body.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::{BinaryOp, CastKind, ExprKind, UnaryOp, ValueCategory};
use kiln_ast::stmt::StmtKind;
use kiln_ast::types::{TypeKind, TypeView};
use kiln_ast::{DeclId, ExprId, QualType, StmtId};
use kiln_diag::{Diag, DiagId, FixIt};
use kiln_source::fx::FxHashMap;
use kiln_source::{Loc, Span};

use crate::Sema;
use crate::cfg::{BlockId, Cfg, Elem, Term, edge};

/// clang `ClassifyRefs::Class`, in its order (a reference keeps the
/// greatest class it is given).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Class {
    Init,
    Use,
    SelfInit,
    ConstRefUse,
    Ignore,
}

/// clang `Value`: two bits per variable (`Unknown` is 0).
const INIT: u8 = 1;
const UNINIT: u8 = 2;
const MAY: u8 = 3;
/// Every slot of a word `UNINIT`.
const ALL_UNINIT: u64 = 0xAAAA_AAAA_AAAA_AAAA;
/// Every slot of a word `INIT`.
const ALL_INIT: u64 = 0x5555_5555_5555_5555;

/// clang `UninitUse::Kind`, in its order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum UseKind {
    Maybe,
    Sometimes,
    AfterDecl,
    AfterCall,
    Always,
}

/// clang `UninitUse::Branch`: the terminator of a block (or the case
/// label of a switch's successor) whose `output`th edge is taken while
/// the variable is uninitialized.
#[derive(Clone, Copy, Debug)]
struct Branch {
    term: Term,
    label: Option<StmtId>,
    output: u32,
}

/// clang `UninitUse`.
#[derive(Clone, Debug)]
struct Use {
    user: ExprId,
    always: bool,
    after_call: bool,
    after_decl: bool,
    branches: Vec<Branch>,
}

impl Use {
    fn new(user: ExprId, always: bool) -> Use {
        Use { user, always, after_call: false, after_decl: false, branches: Vec::new() }
    }

    fn kind(&self) -> UseKind {
        if self.always {
            UseKind::Always
        } else if self.after_call {
            UseKind::AfterCall
        } else if self.after_decl {
            UseKind::AfterDecl
        } else if !self.branches.is_empty() {
            UseKind::Sometimes
        } else {
            UseKind::Maybe
        }
    }
}

/// One variable's entry of clang's `UninitValsDiagReporter` maps.
struct Uses {
    var: u32,
    list: Vec<Use>,
    self_init: bool,
}

/// clang `UninitValsDiagReporter`: the uses and the const-reference
/// uses per variable, in first-report order.
#[derive(Default)]
struct Reports {
    uses: Vec<Uses>,
    const_ref: Vec<Uses>,
}

impl Reports {
    fn entry(map: &mut Vec<Uses>, var: u32) -> &mut Uses {
        let i = map.iter().position(|u| u.var == var).unwrap_or(map.len());
        if i == map.len() {
            map.push(Uses { var, list: Vec::new(), self_init: false });
        }
        &mut map[i]
    }
}

/// The analysis' reusable buffers (cleared, not freed, between bodies).
#[derive(Default, Debug)]
pub(crate) struct Scratch {
    /// The tracked variables; a variable's index is its position.
    vars: Vec<DeclId>,
    /// Each tracked variable's declaration statement (its begin loc).
    var_stmts: Vec<StmtId>,
    index: FxHashMap<DeclId, u32>,
    class: FxHashMap<ExprId, Class>,
    /// `u64` words per value row.
    words: usize,
    /// The value row at each block's exit (`blocks * words`).
    vals: Vec<u64>,
    /// The block being run (clang's scratch vector).
    cur: Vec<u64>,
    analyzed: Vec<bool>,
    visited: Vec<bool>,
    queued: Vec<bool>,
    had_use: Vec<bool>,
    /// Each block's reverse post-order number (`u32::MAX`: unreached).
    rpo_num: Vec<u32>,
    rpo: Vec<BlockId>,
    dfs: Vec<(BlockId, u32)>,
    heap: BinaryHeap<Reverse<(u32, u32)>>,
    /// `getUninitUse`'s successor counts and queue.
    sv: Vec<u32>,
    queue: Vec<BlockId>,
    kids: Vec<ExprId>,
}

/// Where a run reports to: the pruning pass (which blocks had a use) or
/// the reporter.
enum Sink<'s> {
    Prune { had_any: bool },
    Report(&'s mut Reports),
}

impl Sema<'_, '_> {
    /// Notes that the body being parsed needs the analysis: `d` is a local
    /// variable without an initializer (yet) — declared without one, or
    /// named inside its own initializer.
    pub(crate) fn note_uninit_var(&mut self, d: DeclId) {
        if self.ast.decls.var(d).is_some_and(|v| v.init.is_none())
            && self.has_local_storage(d)
            && let Some(f) = self.func.as_mut()
        {
            f.uninit_vars = true;
        }
    }

    /// Whether any of the analysis' warnings is enabled at function `f`
    /// (clang checks the four at `D->getBeginLoc()`).
    pub(crate) fn uninit_enabled(&self, f: DeclId) -> bool {
        let at = self.ast.decls.get(f).loc();
        [
            DiagId::sema_uninit_var,
            DiagId::sema_sometimes_uninit_var,
            DiagId::sema_maybe_uninit_var,
            DiagId::sema_uninit_const_reference,
        ]
        .into_iter()
        .any(|id| self.diags.is_enabled_at(id, at))
    }

    /// clang `runUninitializedVariablesAnalysis` with an
    /// `UninitValsDiagReporter` over the graph of `f`'s `body`.
    pub(crate) fn check_uninitialized(&mut self, f: DeclId, body: StmtId) {
        let Some(cfg) = self.cfg_take(body) else { return };
        let mut s = std::mem::take(&mut self.uninit_scratch);
        let reports = Run { sema: self, cfg: &cfg, s: &mut s }.analyze();
        if let Some(r) = reports {
            self.uninit_flush(f, &s, r);
        }
        self.uninit_scratch = s;
        self.cfg_put(cfg);
    }

    /// clang `isTrackedVar`, for a variable a declaration statement of
    /// the body declares: an automatic, non-implicit variable of scalar,
    /// vector or non-empty class type (a class variable is always
    /// initialized by its declaration in C++, so only its own
    /// initializer can see it uninitialized).
    fn uninit_tracked(&self, d: DeclId) -> bool {
        if self.kind_of(d) != DeclKind::Var {
            return false;
        }
        let n = self.ast.decls.get(d);
        if n.has(DeclFlags::IMPLICIT) || !self.has_local_storage(d) {
            return false;
        }
        let Some(t) = n.ty() else { return false };
        let types = &self.ast.types;
        if types.is_dependent(t) || types.is_reference(t) {
            return false;
        }
        if let Some(r) = types.as_record(t) {
            return self.uninit_record_not_empty(r, 0);
        }
        self.is_scalar(t) || self.is_vector_type(t)
    }

    /// clang `recordIsNotEmpty`: a field other than an unnamed bit-field,
    /// a zero-size field or a field of empty class type (bases do not
    /// count).
    fn uninit_record_not_empty(&self, r: DeclId, depth: u32) -> bool {
        let Some(def) = self.ast.definition(r) else { return false };
        if depth > 64 {
            return true;
        }
        self.ast.decls.members(def).any(|m| {
            let n = self.ast.decls.get(m);
            if n.kind() != DeclKind::Field
                || (n.name().is_empty()
                    && self.ast.decls.field(m).is_some_and(|f| f.bit_width.is_some()))
                || self.is_zero_size_field(m)
            {
                return false;
            }
            match n.ty().and_then(|t| self.ast.types.as_record(t)) {
                Some(fr) => self.uninit_record_not_empty(fr, depth + 1),
                None => true,
            }
        })
    }

    /// clang `stripCasts`: parentheses, `__extension__`, a chosen
    /// `__builtin_choose_expr` operand, a substituted non-type template
    /// parameter, identity casts, pointer/integer casts of the same width,
    /// and lvalue bit casts.
    fn uninit_strip_casts(&self, mut e: ExprId) -> ExprId {
        let ex = &self.ast.exprs;
        for _ in 0..256 {
            let n = ex.get(e);
            let k = n.kind();
            e = match k {
                ExprKind::ParenExpr => ex.operand(e),
                ExprKind::UnaryOperator if ex.unary_op(e) == UnaryOp::Extension => ex.operand(e),
                ExprKind::ChooseExpr => match ex.choose_chosen(e) {
                    Some(c) => c,
                    None => break,
                },
                ExprKind::SubstNonTypeTemplateParmExpr => ex.subst_replacement(e),
                _ if k.is_cast() => {
                    let sub = ex.operand(e);
                    if ex.cast_kind(e) == CastKind::LValueBitCast
                        || self.uninit_noop_cast(n.ty(), ex.get(sub).ty())
                    {
                        sub
                    } else {
                        break;
                    }
                }
                _ => break,
            };
        }
        e
    }

    /// clang `IgnoreNoopCastsSingleStep`: an identity cast, or a cast
    /// between pointer/integral types of the same size.
    fn uninit_noop_cast(&self, to: QualType, from: QualType) -> bool {
        let types = &self.ast.types;
        if types.same_unqualified(to, from) {
            return true;
        }
        let pi = |t: QualType| types.is_pointer(t) || types.is_integral(t);
        if !pi(to) || !pi(from) {
            return false;
        }
        let size = |t| crate::target::size_align(&self.ast, t, self.abi()).map(|x| x.0);
        matches!((size(to), size(from)), (Some(a), Some(b)) if a == b)
    }

    /// clang `getCalleeDecl` of a call.
    fn uninit_callee_decl(&self, call: ExprId) -> Option<DeclId> {
        let c = self.ignore_paren_imp_casts(self.ast.exprs.callee(call));
        matches!(self.ast.exprs.get(c).kind(), ExprKind::DeclRefExpr | ExprKind::MemberExpr)
            .then(|| self.ast.exprs.decl(c))
    }

    /// clang `hasTrivialBody` of a call's direct callee: its body (the
    /// template pattern's, for a specialization) is `{}`.
    fn uninit_trivial_body(&self, call: ExprId) -> bool {
        let Some(mut f) = self.uninit_callee_decl(call) else { return false };
        if !self.kind_of(f).is_function() {
            return false;
        }
        if let Some((_, info)) = self.ast.spec_info(f)
            && let Some(p) = self.pattern_of(info.template)
        {
            f = p;
        }
        let def = self.ast.definition(f).unwrap_or(f);
        let Some(body) = self.ast.decls.function(def).and_then(|x| x.body) else { return false };
        self.ast.stmts.get(body).kind() == StmtKind::CompoundStmt
            && self.ast.lists.len(self.ast.stmts.compound_body(body)) == 0
    }

    /// clang's `returns_twice` test (the attribute, or one of the C
    /// library functions clang gives it; `kiln_codegen`'s
    /// `is_returns_twice`).
    fn uninit_returns_twice(&self, d: DeclId) -> bool {
        let first = self.ast.first(d);
        if self.ast.inherited_attr(first, AttrKind::RETURNS_TWICE).is_some() {
            return true;
        }
        self.kind_of(d) == DeclKind::Function
            && kiln_abi::mangle::is_extern_c(&self.ast, first)
            && ["setjmp", "_setjmp", "sigsetjmp", "__sigsetjmp", "savectx", "vfork", "getcontext"]
                .contains(&kiln_abi::mangle::decl_name(&self.ast, first).as_str())
    }

    /// clang `typeIsPostfix` over the type as written: a declarator whose
    /// type ends after the name (an array or function type, possibly
    /// under pointers, references or member pointers).
    fn uninit_type_is_postfix(&self, mut q: QualType) -> bool {
        let types = &self.ast.types;
        for _ in 0..64 {
            q = match types.view(q.ty()) {
                TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => p,
                TypeView::MemberPointer { pointee, .. } => pointee,
                _ => {
                    return match types.kind(q.ty()) {
                        TypeKind::ConstantArray
                        | TypeKind::IncompleteArray
                        | TypeKind::VariableArray
                        | TypeKind::DependentSizedArray
                        | TypeKind::QualifiedArray
                        | TypeKind::Function => true,
                        TypeKind::ExtQual => {
                            q = types.desugar_once(q);
                            continue;
                        }
                        _ => false,
                    };
                }
            };
        }
        false
    }

    /// clang `VarDecl::getEndLoc` of a variable without an initializer:
    /// the name, or the end of a postfix declarator.
    fn uninit_var_end(&self, v: DeclId) -> Loc {
        let n = self.ast.decls.get(v);
        if n.ty().is_some_and(|t| self.uninit_type_is_postfix(t))
            && let Some(&end) = self.ast.decls.cold.declarator_end.get(v)
        {
            return end;
        }
        n.loc()
    }

    /// clang `Stmt::getEndLoc` for the statements a branch fix-it spans
    /// (and a discarded `if constexpr` branch's, `templ/subst/stmts.rs`).
    pub(crate) fn uninit_stmt_end(&self, s: StmtId) -> Loc {
        let st = &self.ast.stmts;
        let n = st.get(s);
        match n.kind() {
            StmtKind::CompoundStmt => st.rbrace(s),
            StmtKind::IfStmt => {
                let d = st.if_data(s);
                match d.els.or(d.then) {
                    Some(x) => self.uninit_stmt_end(x),
                    None => n.loc(),
                }
            }
            StmtKind::WhileStmt | StmtKind::ForStmt => {
                st.loop_data(s).body.map_or(n.loc(), |b| self.uninit_stmt_end(b))
            }
            StmtKind::DoStmt => st.do_rparen(s),
            StmtKind::CXXForRangeStmt => self.uninit_stmt_end(st.range_for_data(s).body),
            StmtKind::ExprStmt => self.expr_end_loc(st.expr_stmt_expr(s)),
            StmtKind::ReturnStmt => st.return_value(s).map_or(n.loc(), |v| self.expr_end_loc(v)),
            StmtKind::CaseStmt => self.uninit_stmt_end(st.case_data(s).sub),
            StmtKind::DefaultStmt => self.uninit_stmt_end(st.default_sub(s)),
            StmtKind::LabelStmt | StmtKind::AttributedStmt => self.uninit_stmt_end(st.sub_stmt(s)),
            StmtKind::GotoStmt => self.uninit_next_token(n.loc()).unwrap_or(n.loc()),
            _ => n.loc(),
        }
    }

    /// The start of the token after the one at `at` (separated by
    /// whitespace only): a `goto`'s label, which the AST does not locate.
    fn uninit_next_token(&self, at: Loc) -> Option<Loc> {
        if at.is_macro() {
            return None;
        }
        let end = self.token_end(at)?;
        let sm = self.diags.sm();
        let (file, pos) = sm.offset_of(end)?;
        let text = sm.text(file).as_bytes();
        let skip = text.get(pos as usize..)?.iter().take_while(|c| c.is_ascii_whitespace()).count();
        Some(end.with_offset(skip as u32))
    }

    /// A token range `[lo, end of the token at hi]`.
    fn uninit_tok_span(&self, lo: Loc, hi: Loc) -> Span {
        Span::new(lo, self.token_end(hi).unwrap_or(hi))
    }

    /// `UninitValsDiagReporter::flushDiagnostics`.
    fn uninit_flush(&self, f: DeclId, s: &Scratch, r: Reports) {
        for u in r.uses {
            let (v, stmt) = (s.vars[u.var as usize], s.var_stmts[u.var as usize]);
            if !u.list.is_empty() && u.self_init && Self::uninit_has_always(&u.list) {
                self.uninit_self_init_use(f, v, stmt);
                continue;
            }
            let mut list = u.list;
            list.sort_by(|a, b| {
                b.kind().cmp(&a.kind()).then(self.begin_loc(a.user).cmp(&self.begin_loc(b.user)))
            });
            for one in list {
                let one = if u.self_init { Use::new(one.user, false) } else { one };
                if self.uninit_diagnose_use(f, v, stmt, &one, false) {
                    break;
                }
            }
        }
        for u in r.const_ref {
            let (v, stmt) = (s.vars[u.var as usize], s.var_stmts[u.var as usize]);
            if !u.list.is_empty() && u.self_init && Self::uninit_has_always(&u.list) {
                self.uninit_self_init_use(f, v, stmt);
                continue;
            }
            if let Some(first) = u.list.first() {
                let msg = self
                    .diag(DiagId::sema_uninit_const_reference, self.begin_loc(first.user))
                    .arg(self.decl_arg(v))
                    .range(self.uninit_user_range(first.user));
                self.emit(msg);
            }
        }
    }

    /// The highlighted range of a use: a `DeclRefExpr`'s qualifier and name.
    fn uninit_user_range(&self, user: ExprId) -> Span {
        if self.ast.exprs.get(user).kind() == ExprKind::DeclRefExpr {
            self.self_ref_dre_range(user)
        } else {
            self.expr_range(user)
        }
    }

    /// clang `hasAlwaysUninitializedUse`.
    fn uninit_has_always(uses: &[Use]) -> bool {
        uses.iter()
            .any(|u| matches!(u.kind(), UseKind::Always | UseKind::AfterCall | UseKind::AfterDecl))
    }

    /// The self-initialized variable's own initializer reported as an
    /// always-uninitialized use.
    fn uninit_self_init_use(&self, f: DeclId, v: DeclId, stmt: StmtId) {
        if let Some(init) = self.ast.decls.var(v).and_then(|x| x.init) {
            let user = self.ignore_paren_casts(init);
            self.uninit_diagnose_use(f, v, stmt, &Use::new(user, true), true);
        }
    }

    /// clang `DiagnoseUninitializedUse`.
    fn uninit_diagnose_use(
        &self,
        f: DeclId,
        v: DeclId,
        stmt: StmtId,
        u: &Use,
        always_report: bool,
    ) -> bool {
        let user = u.user;
        let name = self.decl_arg(v);
        if self.ast.exprs.get(user).kind() == ExprKind::DeclRefExpr
            && let Some(init) = self.ast.decls.var(v).and_then(|x| x.init)
        {
            if !always_report && user == self.ignore_paren_imp_casts(init) {
                return false;
            }
            if self.uninit_contains(init, user) {
                let msg =
                    self.diag(DiagId::sema_uninit_self_reference_in_init, self.begin_loc(user))
                        .arg(name)
                        .range(self.uninit_tok_span(
                            self.ast.decls.get(v).loc(),
                            self.ast.decls.get(v).loc(),
                        ))
                        .range(self.uninit_user_range(user));
                self.emit(msg);
                return true;
            }
        }
        let mut diags = self.uninit_diag_use(f, v, stmt, u, &name);
        let note = match self.uninit_init_fixit(v, &name) {
            Some(n) => n,
            None => self.diag(DiagId::note_var_declared_here, self.stmt_begin_loc(stmt)).arg(name),
        };
        if let Some(last) = diags.pop() {
            diags.push(last.with_note(note));
        }
        for d in diags {
            self.emit(d);
        }
        true
    }

    /// clang `ContainsReference`: `needle` is in the tree of `e`.
    fn uninit_contains(&self, e: ExprId, needle: ExprId) -> bool {
        if e == needle {
            return true;
        }
        if self.ast.exprs.get(e).kind() == ExprKind::StmtExpr {
            // clang's walk enters the statement-expression body, which
            // `children` does not: decide by source containment.
            let (b, end, at) = (self.begin_loc(e), self.expr_end_loc(e), self.begin_loc(needle));
            return !b.is_macro() && !end.is_macro() && !at.is_macro() && b <= at && at <= end;
        }
        self.ast
            .exprs
            .children(e, &self.ast.lists)
            .into_iter()
            .any(|c| self.uninit_contains(c, needle))
    }

    /// clang `SuggestInitializationFixit`: the `= 0` note.
    fn uninit_init_fixit(&self, v: DeclId, name: &str) -> Option<Diag> {
        if self.ast.decls.var(v).is_some_and(|x| x.init.is_some()) {
            return None;
        }
        let end = self.uninit_var_end(v);
        if end.is_macro() {
            return None;
        }
        let at = self.token_end(end)?;
        let t = self.ast.types.canonical(self.ast.decls.get(v).ty()?);
        let text = if self.is_vector_type(t) {
            String::new()
        } else if self.is_member_pointer(t) {
            " = nullptr".to_string()
        } else {
            self.zero_initializer_fixit(t, None)
        };
        if text.is_empty() {
            return None;
        }
        Some(
            self.diag(DiagId::note_var_fixit_add_initialization, at)
                .arg(name)
                .fixit(FixIt::insert(at, text)),
        )
    }

    /// clang `DiagUninitUse`: the warnings (each with its notes) for one
    /// use; the caller adds the initialization note to the last.
    fn uninit_diag_use(
        &self,
        f: DeclId,
        v: DeclId,
        stmt: StmtId,
        u: &Use,
        name: &str,
    ) -> Vec<Diag> {
        let user = u.user;
        let user_at = self.begin_loc(user);
        let user_range = self.uninit_user_range(user);
        let use_note = || self.diag(DiagId::note_uninit_var_use, user_at).arg(0).range(user_range);
        let mut out = Vec::new();
        match u.kind() {
            UseKind::Always => {
                out.push(
                    self.diag(DiagId::sema_uninit_var, user_at).arg(name).arg(0).range(user_range),
                );
                return out;
            }
            UseKind::AfterDecl | UseKind::AfterCall => {
                let which = if u.kind() == UseKind::AfterDecl { 4 } else { 5 };
                let dc = if self.lambda_of_call_op(f).is_some() {
                    "lambda expression".to_string()
                } else {
                    format!("function '{}'", self.qualified_decl_name(f))
                };
                let at = self.ast.decls.get(v).loc();
                let range = self.uninit_tok_span(self.stmt_begin_loc(stmt), self.uninit_var_end(v));
                let msg = self
                    .diag(DiagId::sema_sometimes_uninit_var, at)
                    .arg(name)
                    .arg(0)
                    .arg(sometimes_select(which))
                    .arg(dc)
                    .arg("")
                    .range(range)
                    .with_note(use_note());
                out.push(msg);
                return out;
            }
            UseKind::Maybe | UseKind::Sometimes => {}
        }
        for br in &u.branches {
            let Some(b) = self.uninit_branch(br) else { continue };
            let output = br.output as usize;
            let text4 = match b.kind {
                0 => ["true", "false"][output.min(1)],
                1 => ["is entered", "exits because its condition is false"][output.min(1)],
                2 => ["condition is true", "exits because its condition is false"][output.min(1)],
                _ => "",
            };
            let mut msg = self
                .diag(DiagId::sema_sometimes_uninit_var, b.range.lo)
                .arg(name)
                .arg(0)
                .arg(sometimes_select(b.kind))
                .arg(b.str)
                .arg(text4)
                .range(b.range)
                .with_note(use_note());
            if let (Some(remove), Some(f1)) = (b.remove, b.fixit1) {
                let mut note = self
                    .diag(DiagId::note_uninit_fixit_remove_cond, f1.span.lo)
                    .arg(remove)
                    .arg(b.str)
                    .arg(output.min(1))
                    .fixit(f1);
                if let Some(f2) = b.fixit2 {
                    note = note.fixit(f2);
                }
                msg = msg.with_note(note);
            }
            out.push(msg);
        }
        if out.is_empty() {
            out.push(
                self.diag(DiagId::sema_maybe_uninit_var, user_at)
                    .arg(name)
                    .arg(0)
                    .range(user_range),
            );
        }
        out
    }

    /// The `switch` arm of `DiagUninitUse` for one branch: the warning's
    /// selector, the construct's spelling, its range and the removal
    /// note's fix-its; `None` for a terminator clang does not describe.
    #[allow(clippy::too_many_lines)]
    fn uninit_branch(&self, br: &Branch) -> Option<BranchDiag> {
        let st = &self.ast.stmts;
        let ex = &self.ast.exprs;
        let out = br.output != 0;
        let fixit_str = if out { "true" } else { "false" };
        let mut b = BranchDiag {
            kind: 0,
            str: "",
            range: Span::empty(Loc::INVALID),
            remove: None,
            fixit1: None,
            fixit2: None,
        };
        if let Some(label) = br.label {
            match st.get(label).kind() {
                StmtKind::CaseStmt => {
                    b.kind = 3;
                    b.str = "case";
                    b.range = self.expr_range(st.case_data(label).lhs);
                }
                StmtKind::DefaultStmt => {
                    b.kind = 3;
                    b.str = "default";
                    let at = st.get(label).loc();
                    b.range = self.uninit_tok_span(at, at);
                }
                _ => return None,
            }
            return Some(b);
        }
        match br.term {
            Term::If(s) => {
                let d = st.if_data(s);
                let (cond, then) = (d.cond?, d.then?);
                b.str = "if";
                b.range = self.expr_range(cond);
                b.remove = Some(1);
                let if_at = st.get(s).loc();
                let then_at = self.stmt_begin_loc(then);
                let els = d.els.map(|e| (self.stmt_begin_loc(e), self.uninit_stmt_end(e)));
                let if_end = self.uninit_stmt_end(s);
                self.uninit_if_fixit(
                    &mut b,
                    out,
                    if_at,
                    if_end,
                    then_at,
                    self.uninit_stmt_end(then),
                    els,
                );
            }
            Term::Cond(e) if ex.get(e).kind() == ExprKind::ConditionalOperator => {
                let cond = ExprId::new(ex.get(e).a());
                let [t, f] = ex.cond_arms(e);
                b.str = "?:";
                b.range = self.expr_range(cond);
                b.remove = Some(1);
                let els = Some((self.begin_loc(f), self.expr_end_loc(f)));
                self.uninit_if_fixit(
                    &mut b,
                    out,
                    self.begin_loc(e),
                    self.expr_end_loc(e),
                    self.begin_loc(t),
                    self.expr_end_loc(t),
                    els,
                );
            }
            Term::Logical(e) if ex.get(e).kind() == ExprKind::BinaryOperator => {
                let op = ex.binary_op(e);
                if !matches!(op, BinaryOp::LAnd | BinaryOp::LOr) {
                    return None;
                }
                let [l, _] = ex.operands(e);
                b.str = op.spelling();
                b.range = self.expr_range(l);
                b.remove = Some(1);
                b.fixit1 =
                    Some(if (op == BinaryOp::LAnd && out) || (op == BinaryOp::LOr && !out) {
                        FixIt::remove(self.uninit_tok_span(self.begin_loc(e), ex.get(e).loc()))
                    } else {
                        FixIt::replace(self.expr_range(e), fixit_str)
                    });
            }
            Term::Loop(s) => match st.get(s).kind() {
                StmtKind::WhileStmt => {
                    b.kind = 1;
                    b.str = "while";
                    b.range = self.expr_range(st.loop_data(s).cond?);
                    b.remove = Some(0);
                    b.fixit1 = Some(FixIt::replace(b.range, fixit_str));
                }
                StmtKind::ForStmt => {
                    b.kind = 1;
                    b.str = "for";
                    b.range = self.expr_range(st.loop_data(s).cond?);
                    b.remove = Some(0);
                    b.fixit1 = Some(if out {
                        FixIt::remove(b.range)
                    } else {
                        FixIt::replace(b.range, fixit_str)
                    });
                }
                StmtKind::CXXForRangeStmt => {
                    if out {
                        return None;
                    }
                    let range = st.range_for_data(s).range;
                    b.kind = 1;
                    b.str = "for";
                    b.range = self.expr_range(self.ast.decls.var(range).and_then(|x| x.init)?);
                }
                StmtKind::DoStmt => {
                    b.kind = 2;
                    b.str = "do";
                    b.range = self.expr_range(st.loop_data(s).cond?);
                    b.remove = Some(0);
                    b.fixit1 = Some(FixIt::replace(b.range, fixit_str));
                }
                _ => return None,
            },
            _ => return None,
        }
        Some(b)
    }

    /// clang `CreateIfFixit` for an `if` (or `?:`) at `[if_at, if_end]`
    /// whose then-part starts at `then_at` and ends at `then_end`.
    #[allow(clippy::too_many_arguments)]
    fn uninit_if_fixit(
        &self,
        b: &mut BranchDiag,
        out: bool,
        if_at: Loc,
        if_end: Loc,
        then_at: Loc,
        then_end: Loc,
        els: Option<(Loc, Loc)>,
    ) {
        if out {
            b.fixit1 = Some(FixIt::remove(Span::new(if_at, then_at)));
            if let Some((_, else_end)) = els {
                let else_kw = self.token_end(then_end).unwrap_or(then_end);
                b.fixit2 = Some(FixIt::remove(self.uninit_tok_span(else_kw, else_end)));
            }
        } else if let Some((else_at, _)) = els {
            b.fixit1 = Some(FixIt::remove(Span::new(if_at, else_at)));
        } else {
            b.fixit1 = Some(FixIt::remove(self.uninit_tok_span(if_at, if_end)));
        }
    }
}

/// The `sema_sometimes_uninit_var` select index of clang's branch kind
/// (0 `if`/`?:`/logical condition, 1 loop, 2 loop entry, 3 switch case,
/// 4 after the declaration, 5 after a call). kiln's template lists the
/// condition alternative last (`kiln_diag` requires every argument to show in
/// the fallback alternative), so the other kinds shift down by one.
fn sometimes_select(kind: u32) -> u32 {
    (kind + 5) % 6
}

/// One branch's description in `DiagUninitUse`.
struct BranchDiag {
    kind: u32,
    str: &'static str,
    range: Span,
    /// The `note_uninit_fixit_remove_cond` select: 0 removes the condition,
    /// 1 removes the operator `str` (clang's select, in reverse order).
    remove: Option<u32>,
    fixit1: Option<FixIt>,
    fixit2: Option<FixIt>,
}

/// One analysis run over a graph.
struct Run<'r, 'a, 'b> {
    sema: &'r Sema<'a, 'b>,
    cfg: &'r Cfg,
    s: &'r mut Scratch,
}

impl Run<'_, '_, '_> {
    /// The value of variable `i` in row `row` of `w`-word rows of `v`.
    fn get(v: &[u64], words: usize, row: usize, i: u32) -> u8 {
        let w = v[row * words + (i / 32) as usize];
        ((w >> ((i % 32) * 2)) & 3) as u8
    }

    fn cur_get(&self, i: u32) -> u8 {
        Self::get(&self.s.cur, self.s.words, 0, i)
    }

    fn cur_set(&mut self, i: u32, x: u8) {
        let w = &mut self.s.cur[(i / 32) as usize];
        let sh = (i % 32) * 2;
        *w = (*w & !(3u64 << sh)) | (u64::from(x) << sh);
    }

    fn out(&self, b: BlockId, i: u32) -> u8 {
        Self::get(&self.s.vals, self.s.words, b.idx(), i)
    }

    /// The blocks in clang's id order: the exit, the body's blocks from
    /// the last to the first, the entry (cfg/dump.rs numbering).
    fn clang_order(&self) -> impl Iterator<Item = BlockId> + use<> {
        let n = self.cfg.blocks.len() as u32;
        std::iter::once(BlockId::EXIT)
            .chain((2..n).rev().map(BlockId))
            .chain(std::iter::once(BlockId::ENTRY))
    }

    /// clang `findVar`: the tracked variable `e` names, casts stripped.
    fn find_var(&self, e: ExprId) -> Option<u32> {
        let e = self.sema.uninit_strip_casts(e);
        let ex = &self.sema.ast.exprs;
        (ex.get(e).kind() == ExprKind::DeclRefExpr)
            .then(|| self.s.index.get(&ex.decl(e)).copied())
            .flatten()
    }

    /// The tracked variable a `DeclRefExpr` names.
    fn dre_var(&self, e: ExprId) -> Option<u32> {
        self.s.index.get(&self.sema.ast.exprs.decl(e)).copied()
    }

    /// The whole analysis; `None` when nothing is reported.
    fn analyze(mut self) -> Option<Reports> {
        if !self.collect_vars() {
            return None;
        }
        self.classify_all();
        let n = self.cfg.blocks.len();
        let words = self.s.vars.len().div_ceil(32);
        self.s.words = words;
        self.s.vals.clear();
        self.s.vals.resize(n * words, 0);
        self.s.cur.clear();
        self.s.cur.resize(words, 0);
        for w in &mut self.s.vals[BlockId::ENTRY.idx() * words..][..words] {
            *w = ALL_UNINIT;
        }
        for v in
            [&mut self.s.analyzed, &mut self.s.visited, &mut self.s.queued, &mut self.s.had_use]
        {
            v.clear();
            v.resize(n, false);
        }
        self.s.analyzed[BlockId::ENTRY.idx()] = true;
        self.compute_rpo();
        self.s.heap.clear();
        self.enqueue_succs(BlockId::ENTRY);
        let mut sink = Sink::Prune { had_any: false };
        while let Some(Reverse((_, b))) = self.s.heap.pop() {
            let b = BlockId(b);
            self.s.queued[b.idx()] = false;
            let changed = self.run_block(b, &mut sink);
            if changed || !self.s.visited[b.idx()] {
                self.enqueue_succs(b);
            }
            self.s.visited[b.idx()] = true;
        }
        if !matches!(sink, Sink::Prune { had_any: true }) {
            return None;
        }
        let mut reports = Reports::default();
        let mut sink = Sink::Report(&mut reports);
        let order: Vec<BlockId> = self.clang_order().filter(|b| self.s.had_use[b.idx()]).collect();
        for b in order {
            self.run_block(b, &mut sink);
        }
        Some(reports)
    }

    /// clang `computeSetOfDeclarations`, limited to the variables a
    /// declaration statement of the graph declares; `false` when there
    /// are none.
    fn collect_vars(&mut self) -> bool {
        let (sema, cfg) = (self.sema, self.cfg);
        self.s.vars.clear();
        self.s.var_stmts.clear();
        self.s.index.clear();
        let st = &sema.ast.stmts;
        for el in &cfg.elems {
            let Elem::Stmt(s) = *el else { continue };
            if st.get(s).kind() != StmtKind::DeclStmt {
                continue;
            }
            let list = st.decl_stmt_decls(s);
            if sema.ast.lists.len(list) != 1 {
                continue;
            }
            if let Some(d) = sema.ast.lists.at::<DeclId>(list, 0)
                && sema.uninit_tracked(d)
            {
                self.track(d, s);
            }
        }
        for &(at, d) in &cfg.decl_marks {
            if !sema.uninit_tracked(d) {
                continue;
            }
            let stmt = cfg.elems[at as usize..].iter().find_map(|el| match *el {
                Elem::Stmt(s) if st.get(s).kind() == StmtKind::DeclStmt => Some(s),
                _ => None,
            });
            if let Some(s) = stmt {
                self.track(d, s);
            }
        }
        !self.s.vars.is_empty()
    }

    fn track(&mut self, d: DeclId, s: StmtId) {
        if self.s.index.contains_key(&d) {
            return;
        }
        self.s.index.insert(d, self.s.vars.len() as u32);
        self.s.vars.push(d);
        self.s.var_stmts.push(s);
    }

    /// clang `getSelfInitExpr`: the initializer of tracked variable `d`
    /// is, casts stripped, a reference to `d` itself.
    fn self_init_expr(&self, d: DeclId) -> Option<ExprId> {
        let init = self.sema.ast.decls.var(d)?.init?;
        let e = self.sema.uninit_strip_casts(init);
        let ex = &self.sema.ast.exprs;
        (ex.get(e).kind() == ExprKind::DeclRefExpr && ex.decl(e) == d).then_some(e)
    }

    /// `ClassifyRefs` over every element of every block, the commas
    /// (which have no element) and the multi-variable declarations'
    /// variables.
    fn classify_all(&mut self) {
        self.s.class.clear();
        let (sema, cfg) = (self.sema, self.cfg);
        for el in &cfg.elems {
            match *el {
                Elem::Stmt(s) => {
                    let st = &sema.ast.stmts;
                    if st.get(s).kind() == StmtKind::DeclStmt {
                        let list = st.decl_stmt_decls(s);
                        if sema.ast.lists.len(list) == 1
                            && let Some(d) = sema.ast.lists.at::<DeclId>(list, 0)
                        {
                            self.classify_decl(d);
                        }
                    }
                }
                Elem::Expr(e) => {
                    if sema.ast.exprs.get(e).kind() == ExprKind::LambdaExpr {
                        let mut kids = std::mem::take(&mut self.s.kids);
                        kids.clear();
                        self.lambda_nodes(e, &mut kids);
                        for &k in &kids {
                            self.classify_node(k);
                        }
                        self.s.kids = kids;
                    } else {
                        self.classify_node(e);
                    }
                }
                _ => {}
            }
        }
        for &c in &cfg.commas {
            self.classify_node(c);
        }
        for &(_, d) in &cfg.decl_marks {
            self.classify_decl(d);
        }
    }

    /// `ClassifyRefs::VisitDeclStmt` for one variable.
    fn classify_decl(&mut self, d: DeclId) {
        if self.s.index.contains_key(&d)
            && let Some(dre) = self.self_init_expr(d)
        {
            self.s.class.insert(dre, Class::SelfInit);
        }
    }

    /// The post-order nodes of a lambda's capture initializers, which
    /// clang's graph evaluates before the lambda (the operand of `sizeof`
    /// and `alignof` excluded, as the graph excludes it).
    fn lambda_nodes(&self, e: ExprId, out: &mut Vec<ExprId>) {
        let ex = &self.sema.ast.exprs;
        let Some(list) = ex.lambda(e).inits else { return };
        for init in self.sema.ast.lists.iter::<ExprId>(list) {
            self.post_order(init, out);
        }
    }

    fn post_order(&self, e: ExprId, out: &mut Vec<ExprId>) {
        let ex = &self.sema.ast.exprs;
        match ex.get(e).kind() {
            ExprKind::UnaryExprOrTypeTraitExpr => {}
            ExprKind::LambdaExpr => self.lambda_nodes(e, out),
            _ => {
                let mut kids = ex.children(e, &self.sema.ast.lists);
                if ex.get(e).kind() == ExprKind::BinaryOperator
                    && ex.binary_op(e) == BinaryOp::Assign
                {
                    kids.reverse();
                }
                for k in kids {
                    self.post_order(k, out);
                }
            }
        }
        out.push(e);
    }

    /// clang `ClassifyRefs::classify`.
    fn classify(&mut self, e: ExprId, c: Class) {
        let sema = self.sema;
        let ex = &sema.ast.exprs;
        let e = sema.ignore_parens(e);
        match ex.get(e).kind() {
            ExprKind::ConditionalOperator => {
                let [t, f] = ex.cond_arms(e);
                self.classify(t, c);
                self.classify(f, c);
                return;
            }
            ExprKind::BinaryConditionalOperator => {
                let f = ex.binary_conditional_operands(e, &sema.ast.lists)[3];
                self.classify(f, c);
                return;
            }
            ExprKind::OpaqueValueExpr => {
                if let Some(src) = ex.opaque_source(e) {
                    self.classify(src, c);
                }
                return;
            }
            // A member is a field (or a static data member): never the
            // base's variable.
            ExprKind::MemberExpr => return,
            ExprKind::BinaryOperator => {
                let [l, r] = ex.operands(e);
                match ex.binary_op(e) {
                    BinaryOp::PtrMemD | BinaryOp::PtrMemI => self.classify(l, c),
                    BinaryOp::Comma => self.classify(r, c),
                    _ => {}
                }
                return;
            }
            _ => {}
        }
        let s = sema.uninit_strip_casts(e);
        if ex.get(s).kind() == ExprKind::DeclRefExpr && self.s.index.contains_key(&ex.decl(s)) {
            let slot = self.s.class.entry(s).or_insert(Class::Init);
            *slot = (*slot).max(c);
        }
    }

    /// The `ClassifyRefs` visitors for one element.
    fn classify_node(&mut self, e: ExprId) {
        let sema = self.sema;
        let ex = &sema.ast.exprs;
        let n = ex.get(e);
        let k = n.kind();
        match k {
            ExprKind::BinaryOperator => {
                if matches!(ex.binary_op(e), BinaryOp::Assign | BinaryOp::Comma) {
                    self.classify(ex.operands(e)[0], Class::Ignore);
                }
            }
            ExprKind::CompoundAssignOperator => self.classify(ExprId::new(n.a()), Class::Use),
            ExprKind::UnaryOperator => {
                if ex.unary_op(e).is_inc_dec() {
                    self.classify(ex.operand(e), Class::Use);
                }
            }
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr => {
                let args = ex.call_args(e);
                if matches!(k, ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr)
                    && sema.self_ref_is_std_move(e)
                {
                    if let Some(a) = sema.ast.lists.iter::<ExprId>(args).next()
                        && sema.ast.types.as_record(ex.get(a).ty()).is_none()
                    {
                        self.classify(a, Class::Use);
                    }
                    return;
                }
                let trivial = sema.uninit_trivial_body(e);
                for a in sema.ast.lists.iter::<ExprId>(args) {
                    let an = ex.get(a);
                    let ty = an.ty();
                    if an.value_category() != ValueCategory::PRValue {
                        if sema.ast.types.effective_quals(ty).is_const() {
                            self.classify(
                                a,
                                if trivial { Class::Ignore } else { Class::ConstRefUse },
                            );
                        }
                    } else if sema.ast.types.is_pointer(ty)
                        && sema
                            .ast
                            .types
                            .pointee(ty)
                            .is_some_and(|p| sema.ast.types.effective_quals(p).is_const())
                    {
                        let mut x = sema.uninit_strip_casts(a);
                        if ex.get(x).kind() == ExprKind::UnaryOperator
                            && ex.unary_op(x) == UnaryOp::AddrOf
                        {
                            x = ex.operand(x);
                        }
                        self.classify(x, Class::Ignore);
                    }
                }
            }
            _ if k.is_cast() => {
                if ex.cast_kind(e) == CastKind::LValueToRValue {
                    self.classify(ex.operand(e), Class::Use);
                } else if k == ExprKind::CStyleCastExpr && sema.ast.types.is_void(n.ty()) {
                    self.classify(ex.operand(e), Class::Ignore);
                }
            }
            _ => {}
        }
    }

    /// `ClassifyRefs::get`.
    fn class_of(&self, dre: ExprId) -> Class {
        match self.s.class.get(&dre) {
            Some(&c) => c,
            None if self.dre_var(dre).is_some() => Class::Init,
            None => Class::Ignore,
        }
    }

    /// The reverse post-order of the blocks reached from the entry over
    /// live edges (clang `PostOrderCFGView`).
    fn compute_rpo(&mut self) {
        let cfg = self.cfg;
        let n = cfg.blocks.len();
        self.s.rpo_num.clear();
        self.s.rpo_num.resize(n, u32::MAX);
        self.s.rpo.clear();
        self.s.dfs.clear();
        // `visited` doubles as the DFS mark; it is reset below.
        self.s.visited[BlockId::ENTRY.idx()] = true;
        self.s.dfs.push((BlockId::ENTRY, 0));
        while let Some(top) = self.s.dfs.last_mut() {
            let (b, i) = *top;
            let succs = cfg.succs(b);
            if (i as usize) < succs.len() {
                top.1 += 1;
                let a = succs[i as usize];
                if a.flags & edge::LIVE != 0 && !self.s.visited[a.block.idx()] {
                    self.s.visited[a.block.idx()] = true;
                    self.s.dfs.push((a.block, 0));
                }
            } else {
                self.s.rpo.push(b);
                self.s.dfs.pop();
            }
        }
        let total = self.s.rpo.len() as u32;
        for (i, &b) in self.s.rpo.iter().enumerate() {
            self.s.rpo_num[b.idx()] = total - 1 - i as u32;
        }
        self.s.visited.iter_mut().for_each(|v| *v = false);
    }

    /// `ForwardDataflowWorklist::enqueueSuccessors`.
    fn enqueue_succs(&mut self, b: BlockId) {
        for a in self.cfg.succs(b) {
            let t = a.block.idx();
            if a.flags & edge::LIVE != 0 && !self.s.queued[t] && self.s.rpo_num[t] != u32::MAX {
                self.s.queued[t] = true;
                self.s.heap.push(Reverse((self.s.rpo_num[t], a.block.0)));
            }
        }
    }

    /// clang `runOnBlock`: merges the analysed live predecessors, runs
    /// the transfer functions over the elements (and an `asm goto`
    /// terminator) and stores the exit row; `true` if it changed.
    fn run_block(&mut self, b: BlockId, sink: &mut Sink<'_>) -> bool {
        let cfg = self.cfg;
        let words = self.s.words;
        self.s.analyzed[b.idx()] = true;
        let mut first = true;
        for p in cfg.preds(b) {
            if p.flags & edge::LIVE == 0 || !self.s.analyzed[p.block.idx()] {
                continue;
            }
            let row = &self.s.vals[p.block.idx() * words..][..words];
            if first {
                self.s.cur.copy_from_slice(row);
                first = false;
            } else {
                for (c, r) in self.s.cur.iter_mut().zip(row) {
                    *c |= *r;
                }
            }
        }
        if first {
            self.s.cur.iter_mut().for_each(|c| *c = 0);
        }
        let range = cfg.block(b).elems.clone();
        let marks = &cfg.decl_marks;
        let mut m = marks.partition_point(|&(at, _)| at < range.start);
        for i in range.clone() {
            while m < marks.len() && marks[m].0 == i {
                self.transfer_decl(marks[m].1, b, sink);
                m += 1;
            }
            match cfg.elems[i as usize] {
                Elem::Stmt(s) => self.transfer_stmt(s, b, sink),
                Elem::Expr(e) => self.transfer_expr(e, b, sink),
                _ => {}
            }
        }
        if let Term::Asm(s) = cfg.block(b).term {
            self.transfer_asm_goto(s);
        }
        let dst = &mut self.s.vals[b.idx() * words..][..words];
        let changed = dst != self.s.cur.as_slice();
        if changed {
            dst.copy_from_slice(&self.s.cur);
        }
        changed
    }

    /// `TransferFunctions::VisitDeclStmt` for a single-variable
    /// declaration statement (a multi-variable one is split by the
    /// graph's marks).
    fn transfer_stmt(&mut self, s: StmtId, b: BlockId, sink: &mut Sink<'_>) {
        let sema = self.sema;
        let st = &sema.ast.stmts;
        if st.get(s).kind() != StmtKind::DeclStmt {
            return;
        }
        let list = st.decl_stmt_decls(s);
        if sema.ast.lists.len(list) == 1
            && let Some(d) = sema.ast.lists.at::<DeclId>(list, 0)
        {
            self.transfer_decl(d, b, sink);
        }
    }

    fn transfer_decl(&mut self, d: DeclId, _b: BlockId, _sink: &mut Sink<'_>) {
        let Some(&i) = self.s.index.get(&d) else { return };
        let v = if self.self_init_expr(d).is_some() {
            UNINIT
        } else if self.sema.ast.decls.var(d).is_some_and(|x| x.init.is_some()) {
            INIT
        } else {
            UNINIT
        };
        self.cur_set(i, v);
    }

    /// The `TransferFunctions` visitors for one expression element.
    fn transfer_expr(&mut self, e: ExprId, b: BlockId, sink: &mut Sink<'_>) {
        let sema = self.sema;
        let ex = &sema.ast.exprs;
        match ex.get(e).kind() {
            ExprKind::DeclRefExpr => {
                let Some(i) = self.dre_var(e) else { return };
                match self.class_of(e) {
                    Class::Ignore => {}
                    Class::Init => self.cur_set(i, INIT),
                    Class::Use => {
                        let v = self.cur_get(i);
                        if v >= UNINIT {
                            self.report(e, i, v, b, sink, false);
                        }
                    }
                    Class::ConstRefUse => {
                        let v = self.cur_get(i);
                        if v == UNINIT {
                            self.report(e, i, v, b, sink, true);
                        }
                    }
                    Class::SelfInit => match sink {
                        Sink::Prune { had_any } => {
                            *had_any = true;
                            self.s.had_use[b.idx()] = true;
                        }
                        Sink::Report(r) => {
                            Reports::entry(&mut r.uses, i).self_init = true;
                            Reports::entry(&mut r.const_ref, i).self_init = true;
                        }
                    },
                }
            }
            ExprKind::BinaryOperator => {
                if ex.binary_op(e) == BinaryOp::Assign
                    && let Some(i) = self.find_var(ex.operands(e)[0])
                {
                    self.cur_set(i, INIT);
                }
            }
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr => {
                if let Some(d) = sema.uninit_callee_decl(e) {
                    if sema.uninit_returns_twice(d) {
                        self.s.cur.iter_mut().for_each(|c| *c = ALL_INIT);
                    } else if sema.find_attr(d, AttrKind::ANALYZER_NORETURN).is_some() {
                        self.s.cur.iter_mut().for_each(|c| *c = 0);
                    }
                }
            }
            ExprKind::LambdaExpr => {
                let mut kids = std::mem::take(&mut self.s.kids);
                kids.clear();
                self.lambda_nodes(e, &mut kids);
                for &k in &kids {
                    if k != e && ex.get(k).kind() != ExprKind::LambdaExpr {
                        self.transfer_expr(k, b, sink);
                    }
                }
                self.s.kids = kids;
            }
            _ => {}
        }
    }

    /// `TransferFunctions::VisitGCCAsmStmt` for an `asm goto`: an output
    /// not yet initialized may be uninitialized on the indirect edges.
    fn transfer_asm_goto(&mut self, s: StmtId) {
        let sema = self.sema;
        let st = &sema.ast.stmts;
        let ex = &sema.ast.exprs;
        let outputs = st.asm_data(s).num_outputs as usize;
        for op in st.asm_operands(s).iter().take(outputs) {
            let mut x = sema.uninit_strip_casts(op.expr);
            while ex.get(x).kind() == ExprKind::UnaryOperator {
                x = sema.uninit_strip_casts(ex.operand(x));
            }
            if let Some(i) = self.find_var(x)
                && self.cur_get(i) != INIT
            {
                self.cur_set(i, MAY);
            }
        }
    }

    /// `reportUse` / `reportConstRefUse`.
    fn report(
        &mut self,
        user: ExprId,
        i: u32,
        v: u8,
        b: BlockId,
        sink: &mut Sink<'_>,
        const_ref: bool,
    ) {
        match sink {
            Sink::Prune { had_any } => {
                *had_any = true;
                self.s.had_use[b.idx()] = true;
            }
            Sink::Report(r) => {
                let u = self.uninit_use(user, i, v, b);
                let map = if const_ref { &mut r.const_ref } else { &mut r.uses };
                Reports::entry(map, i).list.push(u);
            }
        }
    }

    /// clang `TransferFunctions::getUninitUse`: whether the use in block
    /// `b` is always uninitialized, uninitialized after the declaration
    /// or the call, or uninitialized whenever certain branches are taken.
    fn uninit_use(&mut self, user: ExprId, i: u32, v: u8, b: BlockId) -> Use {
        let mut u = Use::new(user, v == UNINIT);
        if u.always {
            return u;
        }
        let cfg = self.cfg;
        let n = cfg.blocks.len();
        self.s.sv.clear();
        self.s.sv.resize(n, 0);
        self.s.queue.clear();
        self.s.queue.push(b);
        self.s.sv[b.idx()] = cfg.succs(b).len() as u32;
        while let Some(blk) = self.s.queue.pop() {
            if blk == BlockId::ENTRY {
                u.after_call = true;
            }
            for p in cfg.preds(blk) {
                if p.flags & edge::LIVE == 0 {
                    continue;
                }
                let pred = p.block;
                let at_pred = self.out(pred, i);
                if at_pred == INIT {
                    continue;
                }
                if at_pred == MAY && self.out(blk, i) == UNINIT {
                    u.after_decl = true;
                    continue;
                }
                let succs = cfg.succs(pred);
                let sv = &mut self.s.sv[pred.idx()];
                if *sv == 0 {
                    *sv += succs.iter().filter(|a| a.flags & edge::LIVE == 0).count() as u32;
                }
                *sv += 1;
                if *sv == succs.len() as u32 {
                    self.s.queue.push(pred);
                }
            }
        }
        for blk in self.clang_order() {
            let block = cfg.block(blk);
            let succs = cfg.succs(blk);
            let sv = self.s.sv[blk.idx()];
            if block.term == Term::None || sv == 0 || sv >= succs.len() as u32 {
                continue;
            }
            for (k, a) in succs.iter().enumerate() {
                if a.flags & edge::LIVE == 0
                    || self.s.sv[a.block.idx()] < cfg.succs(a.block).len() as u32
                    || self.out(blk, i) != UNINIT
                {
                    continue;
                }
                if let Term::Switch(_) = block.term {
                    let Some(label) = cfg.block(a.block).label else { continue };
                    if !matches!(
                        self.sema.ast.stmts.get(label).kind(),
                        StmtKind::CaseStmt | StmtKind::DefaultStmt
                    ) {
                        continue;
                    }
                    u.branches.push(Branch { term: block.term, label: Some(label), output: 0 });
                } else {
                    u.branches.push(Branch { term: block.term, label: None, output: k as u32 });
                }
            }
        }
        u
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_slots_pack_two_bits() {
        let v = [ALL_UNINIT, 0];
        assert_eq!(Run::get(&v, 1, 0, 0), UNINIT);
        assert_eq!(Run::get(&v, 1, 0, 31), UNINIT);
        assert_eq!(Run::get(&v, 1, 1, 0), 0);
        assert_eq!(Run::get(&[ALL_INIT], 1, 0, 7), INIT);
        assert!(Class::Use < Class::SelfInit && Class::ConstRefUse < Class::Ignore);
        assert!(UseKind::Always > UseKind::AfterCall && UseKind::Sometimes > UseKind::Maybe);
    }
}
