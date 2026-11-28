//! Stage 6 of the written-reference table (spec/ast.md §18.13, WP-2820):
//! the operator token of an operator expression whose overload resolution
//! selected a function, and the ud-suffix of a user-defined literal. The
//! parser calls these with the expression it just built at the token; the
//! facts are read from that expression (no sema side state), and no entry
//! is made during instantiation or when recording is off (one branch).
//!
//! Stage 6b (WP-2910): a dependent operator in a template pattern gets a
//! `DEPENDENT` entry with its definition-context found set, and each
//! instantiation that reaches the token a per-specialization row read off
//! the rebuilt expression (`note_written_spec_oper`, and per application
//! of a fold-expression's expansion, `note_written_spec_fold`).
//!
//! Stage 6c (WP-2911): the `new` and `delete` keywords of new- and
//! delete-expressions naming the allocation or deallocation function
//! selected, and the `co_await` keyword naming the `operator co_await`
//! applied (`note_written_new_delete`, `note_written_coawait`), with the
//! same dependent entries and rows.

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{BinaryOp, CastKind, ExprDeps, ExprFlags, ExprKind, UnaryOp};
use kiln_ast::ids::RawId;
use kiln_ast::names::{DeclName, OverloadedOperator as OO};
use kiln_ast::written_refs::{
    AdlSet, FOUND_ADL, FoundDecl, PatternNode, REF_BUILTIN, REF_DEPENDENT, REF_REVERSED,
    REF_REWRITTEN, REF_UNRESOLVED, RefLookup, RefQualifier, RefRole, SPEC_BUILTIN, WrittenRefId,
};
use kiln_ast::{DeclId, ExprId};
use kiln_source::{Loc, Span};

use crate::Sema;
use crate::written_keys::RowKeys;

/// What one dependent operator token selected in one specialization
/// (spec/ast.md §18.13.6): the rows it gets.
#[derive(Clone, Debug)]
enum OperSel {
    /// A function (`found` when it differs), with the row's flags
    /// (`REWRITTEN`, `REVERSED`), the `how` its lookup says, its
    /// lookup-context (naming) class for `Member`/`Qualified` class lookups,
    /// and its associated set when ADL ran.
    Fn((DeclId, Option<DeclId>), u8, Option<RefLookup>, Option<DeclId>, Option<AdlSet>),
    /// A built-in candidate or no operator function at all, with its
    /// associated set when operator overload resolution ran ADL on a class
    /// or enumeration operand (WP-3701).
    Builtin(Option<AdlSet>),
    /// Overload resolution failed (diagnosed).
    Unresolved,
    /// Still dependent (a generic lambda's body rebuilt by the enclosing
    /// template's instantiation): the rebuilt node stands for the entry.
    Dependent,
}

impl Sema<'_, '_> {
    /// Records the operator token `tok` of the operator expression `e`
    /// the parser just built (spec/ast.md §18.13): `at` is the location
    /// sema built the operator call at (the token's own, except for the
    /// comma of a C-style cast's parenthesized operand, built at the `(`
    /// as clang does), `close` the `]` or `)` of a subscript or call
    /// (`Loc::INVALID` otherwise), `operands` the operands as written (the
    /// callee object first for a call). An overloaded operator gets an
    /// `operator` entry naming the selected function (two for a
    /// comparison rewritten through `operator<=>` whose comparison with
    /// zero is overloaded); a built-in one none; a failed resolution with
    /// a class or enumeration operand an `UNRESOLVED` entry. A dependent
    /// operator in a template pattern that could select a function gets a
    /// `DEPENDENT` entry, its rows made by each instantiation (§18.13.6).
    pub fn note_written_operator(
        &mut self,
        e: ExprId,
        tok: Span,
        at: Loc,
        close: Loc,
        operands: &[ExprId],
    ) {
        if !self.records_written_refs() || !self.inst.stack.is_empty() {
            return;
        }
        let x = self.written_oper_peel(e);
        let n = *self.ast.exprs.get(x);
        if n.is_type_dependent() && !self.is_err_expr(x) {
            if self.written_oper_may_select(x, close, operands) {
                self.push_written_dependent_oper(x, 0, tok, close);
            }
            return;
        }
        match n.kind() {
            ExprKind::CXXOperatorCallExpr if n.loc() == at => {
                self.push_written_oper_call(x, tok, close, 0, operands);
            }
            ExprKind::CXXRewrittenBinaryOperator if n.loc() == at => {
                self.note_written_rewritten(x, tok);
            }
            ExprKind::CallExpr => {
                // A surrogate call function ([over.call.object]/2): the
                // callee is the object implicitly converted to a pointer
                // to function.
                if let Some((d, f, _)) = self.written_surrogate(self.ast.exprs.callee(x)) {
                    self.push_written_oper(tok, (d, f), close, 0);
                }
            }
            _ if self.is_err_expr(x) => self.note_written_oper_failure(tok, close, operands),
            _ => self.note_written_oper_builtin(x, tok, at, close, operands),
        }
    }

    /// Records the `->` at `tok` of the class member access `e` whose
    /// object expression was `base`: one entry per `operator->` the
    /// chain applied, in application order ([over.ref]/1, spec/ast.md
    /// §18.13); `UNRESOLVED` when `base` has class type and no
    /// `operator->` was applied (diagnosed); nothing for a built-in `->`.
    /// A `base` of a dependent type that could be a class gets a
    /// `DEPENDENT` entry, with no found set ([over.ref]: member
    /// candidates only) and a row per `operator->` each instantiation
    /// applies (§18.13.6).
    pub fn note_written_arrow(&mut self, e: ExprId, base: ExprId, tok: Span) {
        if !self.records_written_refs() || !self.inst.stack.is_empty() {
            return;
        }
        if self.ast.exprs.get(base).is_type_dependent() {
            let x = self.written_oper_peel(e);
            if !self.contains_errors(base)
                && !self.is_err_expr(x)
                && self.is_dependent_overloadable(self.ety(base))
            {
                self.push_written_dependent_oper(x, 0, tok, Loc::INVALID);
            }
            return;
        }
        if self.ast.types.as_record(self.ety(base)).is_none() || self.contains_errors(base) {
            return;
        }
        // The outermost `operator->` call at the token: the member
        // access's object, or within a failed access's recovery.
        let mut chain = Vec::new();
        let mut cur = self.written_arrow_outermost(e, tok.lo);
        while let Some(c) = cur.map(|c| self.written_oper_peel(c))
            && self.ast.exprs.get(c).kind() == ExprKind::CXXOperatorCallExpr
            && self.ast.exprs.operator_call_op(c) == OO::Arrow
            && self.ast.exprs.get(c).loc() == tok.lo
        {
            chain.push(c);
            cur = self.ast.lists.iter::<ExprId>(self.ast.exprs.call_args(c)).next();
        }
        if chain.is_empty() {
            self.push_written_oper_unresolved(tok, Loc::INVALID);
            return;
        }
        // The outermost call is the last applied.
        for &c in chain.iter().rev() {
            self.push_written_oper_call(c, tok, Loc::INVALID, 0, &[]);
        }
    }

    /// The first `operator->` call at `at` in a breadth-first walk of
    /// `e` (bounded: a member access's object is its first child).
    fn written_arrow_outermost(&self, e: ExprId, at: Loc) -> Option<ExprId> {
        let ex = &self.ast.exprs;
        let mut queue = std::collections::VecDeque::from([e]);
        let mut kids = Vec::new();
        for _ in 0..64 {
            let c = queue.pop_front()?;
            let n = ex.get(c);
            if n.kind() == ExprKind::CXXOperatorCallExpr
                && n.loc() == at
                && ex.operator_call_op(c) == OO::Arrow
            {
                return Some(c);
            }
            kids.clear();
            ex.children_into(c, &self.ast.lists, &mut kids);
            queue.extend(kids.iter().copied());
        }
        None
    }

    /// Records the ud-suffixes `suffixes` (each the suffix within its
    /// literal token, one per suffixed piece of a string concatenation)
    /// of the user-defined literal `e` the parser just built: role
    /// `ud-suffix`, the literal operator (a literal operator template's
    /// specialization, `found` the template) or `UNRESOLVED` when none
    /// matched (spec/ast.md §18.13.4).
    pub fn note_written_udl(&mut self, e: ExprId, suffixes: &[Span]) {
        if !self.records_written_refs() || !self.inst.stack.is_empty() {
            return;
        }
        let x = self.written_oper_peel(e);
        let target = if self.ast.is_udl_call(x) {
            self.written_callee(self.ast.exprs.callee(x))
        } else {
            None
        };
        let (entity, found, flags) = match target {
            Some((d, f)) => (Some(d), f, 0),
            None => (None, None, REF_UNRESOLVED),
        };
        for &range in suffixes {
            let r = self.push_written_ref(
                range,
                entity,
                found,
                RefRole::UdSuffix,
                RefQualifier::NONE,
                flags,
            );
            debug_assert!(r.is_some(), "recording is on");
        }
    }

    /// Records the keyword of the new- or delete-expression `e` the parser
    /// just built (spec/ast.md §18.13.6, stage 6c): `tok` the `new`
    /// keyword, or `delete` (`delete[]` as one range over its three
    /// tokens), `colon` the `::` of `::new` / `::delete` (the entry's
    /// qualifier is then `GLOBAL`, the `::` its lead). The entry names the
    /// allocation function ([expr.new]/12) or deallocation function
    /// ([expr.delete]/9-11) the expression selected, `how=qualified`
    /// (looked up in the class's scope, then the global scope); the
    /// deallocation function a new-expression calls when the
    /// initialization throws and the destructor a `delete` calls have no
    /// token and no entry. A dependent one (its type or an operand
    /// dependent, so nothing was selected) gets a `DEPENDENT` entry with no
    /// found set and a row per specialization; a failed one whose operands
    /// are not erroneous an `UNRESOLVED` entry.
    pub fn note_written_new_delete(&mut self, e: ExprId, tok: Span, colon: Option<Loc>) {
        if !self.records_written_refs() || !self.inst.stack.is_empty() {
            return;
        }
        let x = self.written_oper_peel(e);
        let mut kids = Vec::new();
        self.ast.exprs.children_into(x, &self.ast.lists, &mut kids);
        if kids.iter().any(|&k| self.contains_errors(k)) {
            return;
        }
        let q = match colon {
            Some(c) => self.ast.written_refs.global_link(c),
            None => RefQualifier::NONE,
        };
        if self.is_err_expr(x) {
            self.push_written_ref(tok, None, None, RefRole::Operator, q, REF_UNRESOLVED);
            return;
        }
        match self.written_alloc_fn(x) {
            Some(f) => {
                let target = self.written_fn_found(f);
                let id =
                    self.push_written_ref(tok, Some(target.0), target.1, RefRole::Operator, q, 0);
                if let Some(id) = id {
                    self.ast.written_refs.set_how(id, RefLookup::Qualified);
                    self.ast.written_refs.set_using_directive(id, None);
                }
            }
            None if self.written_alloc_dependent(x) => {
                if let Some(id) =
                    self.push_written_ref(tok, None, None, RefRole::Operator, q, REF_DEPENDENT)
                {
                    self.ast.written_refs.note_pattern_node(PatternNode::Oper(x, 0), id);
                }
            }
            _ => {}
        }
    }

    /// Records the `co_await` keyword `tok` of the await-expression `e`
    /// the parser just built ([expr.await]/3.3, spec/ast.md §18.13.6): an
    /// entry naming the `operator co_await` applied (member or not) to the
    /// operand, or to the result of the promise's `await_transform`; none
    /// when no `operator co_await` applies (the awaitable is the awaiter).
    /// The `await_transform`, `await_ready`, `await_suspend` and
    /// `await_resume` calls, `co_yield` and the initial and final suspend
    /// points have no token of their own: no entry. In a coroutine
    /// template (a `DependentCoawaitExpr`, its promise known only per
    /// specialization) a `DEPENDENT` entry with the definition's
    /// unqualified lookup of `operator co_await` as its found set (open to
    /// argument-dependent lookup) and a row per specialization; a failed
    /// one whose operand is not erroneous an `UNRESOLVED` entry.
    pub fn note_written_coawait(&mut self, e: ExprId, tok: Span) {
        if !self.records_written_refs() || !self.inst.stack.is_empty() {
            return;
        }
        let x = self.written_oper_peel(e);
        let mut kids = Vec::new();
        self.ast.exprs.children_into(x, &self.ast.lists, &mut kids);
        match self.ast.exprs.get(x).kind() {
            ExprKind::DependentCoawaitExpr => {
                let o = self.ast.exprs.coawait_operand(x, &self.ast.lists);
                if self.contains_errors(o) {
                    return;
                }
                let Some(id) = self.push_written_ref(
                    tok,
                    None,
                    None,
                    RefRole::Operator,
                    RefQualifier::NONE,
                    REF_DEPENDENT,
                ) else {
                    return;
                };
                self.ast.written_refs.note_pattern_node(PatternNode::Oper(x, 0), id);
                self.note_written_coawait_found(id);
            }
            ExprKind::CoawaitExpr => {
                if let Some(c) = self.written_coawait_call(x) {
                    let adl_args = self.written_call_unconverted_args(c);
                    self.push_written_oper_call(c, tok, Loc::INVALID, 0, &adl_args);
                }
            }
            _ if self.is_err_expr(x) && !kids.iter().any(|&k| self.contains_errors(k)) => {
                self.push_written_oper_unresolved(tok, Loc::INVALID);
            }
            _ => {}
        }
    }

    /// The found set of the dependent `co_await` entry `id`: the
    /// non-member `operator co_await` functions unqualified lookup finds
    /// at the definition ([over.match.oper]/3.2), flag `ADL`.
    fn note_written_coawait_found(&mut self, id: WrittenRefId) {
        let found = self.lookup_operator_name(DeclName::operator(OO::Coawait));
        let mut elems: Vec<FoundDecl> = Vec::new();
        for (&d, &f) in found.decls.iter().zip(found.found.iter()) {
            let target = self.ast.strip_shadow(d);
            let decl = self.ast.first(target);
            if elems.iter().any(|e| e.decl == decl) {
                continue;
            }
            let found = if f != d && f != target { f } else { decl };
            elems.push(FoundDecl { decl, found });
        }
        let s = self.ast.written_refs.intern_found(&elems, RefLookup::Unqualified, FOUND_ADL);
        self.ast.written_refs.set_found(id, s);
    }

    /// The `operator co_await` call of the resolved await-expression `x`
    /// (a `CoawaitExpr`): its awaiter when that is an `operator co_await`
    /// call sema built at the keyword ([expr.await]/3.3), else `None`.
    fn written_coawait_call(&self, x: ExprId) -> Option<ExprId> {
        let ex = &self.ast.exprs;
        let parts = ex.coawait_parts(x, &self.ast.lists)?;
        let c = self.written_oper_peel(parts.common);
        (ex.get(c).kind() == ExprKind::CXXOperatorCallExpr
            && ex.operator_call_op(c) == OO::Coawait
            && ex.get(c).loc() == ex.get(x).loc())
        .then_some(c)
    }

    /// The allocation or deallocation function of the new- or
    /// delete-expression `x`: `None` when it selected none (dependent;
    /// see `written_alloc_dependent`) or `x` is neither.
    fn written_alloc_fn(&self, x: ExprId) -> Option<DeclId> {
        let ex = &self.ast.exprs;
        match ex.get(x).kind() {
            ExprKind::CXXNewExpr => ex.new_extra(x).operator_new,
            ExprKind::CXXDeleteExpr => ex.operator_delete(x),
            _ => None,
        }
    }

    /// Whether the new- or delete-expression `x`, which selected no
    /// function, is dependent: a new-expression whose type or operands
    /// are dependent, a delete-expression whose operand is type-dependent
    /// ([temp.dep.expr]/3).
    fn written_alloc_dependent(&self, x: ExprId) -> bool {
        let ex = &self.ast.exprs;
        match ex.get(x).kind() {
            ExprKind::CXXNewExpr => {
                ex.get(x).deps().intersects(ExprDeps::TYPE_VALUE_INST | ExprDeps::UNEXPANDED_PACK)
            }
            ExprKind::CXXDeleteExpr => ex.get(ex.delete_operand(x)).is_type_dependent(),
            _ => false,
        }
    }

    /// `f` with what lookup found when it differs: the template of an
    /// allocation function template's specialization.
    fn written_fn_found(&self, f: DeclId) -> (DeclId, Option<DeclId>) {
        (f, self.ast.spec_info(f).map(|(_, i)| i.template).filter(|&t| t != f))
    }

    /// Records the operator tokens `toks` (one for a unary fold, two for a
    /// binary one) of the fold-expression `e` the parser just built
    /// ([expr.prim.fold]): each a `DEPENDENT` entry with the fold's
    /// definition-context found set, the rows made per application of
    /// each expansion (spec/ast.md §18.13.6). Nothing when no operand
    /// could have class or enumeration type (a fold over a pack of
    /// `int` template parameters is built-in in every expansion).
    pub fn note_written_fold(&mut self, e: ExprId, toks: &[Span]) {
        if !self.records_written_refs() || !self.inst.stack.is_empty() {
            return;
        }
        let n = *self.ast.exprs.get(e);
        if n.kind() != ExprKind::CXXFoldExpr {
            return;
        }
        let ops = [ExprId::from_raw(n.a()), ExprId::from_raw(n.b())];
        if ops.iter().flatten().any(|&o| self.contains_errors(o))
            || !ops.iter().flatten().any(|&o| self.written_could_overload(o))
        {
            return;
        }
        for (slot, &tok) in (0u8..).zip(toks.iter().take(2)) {
            self.push_written_dependent_oper(e, slot, tok, Loc::INVALID);
        }
    }

    /// True when operand `o` has a class or enumeration type, or a
    /// dependent type that may be one (not a pointer, array, function or
    /// pointer to member): overload resolution may run on it
    /// ([over.match.oper]/1).
    fn written_could_overload(&self, o: ExprId) -> bool {
        self.overloadable_operand(o) || self.is_dependent_overloadable(self.ety(o))
    }

    /// Whether the type-dependent operator expression `x` built from
    /// `operands` (the callee or object first for `()` and `[]`, which
    /// have a `close`) may select a function in some specialization
    /// (spec/ast.md §18.13.6): an operand that may have class or
    /// enumeration type; for `[]`, `()` and `=`, whose candidates are
    /// members only ([over.sub], [over.call], [over.ass]), the object; a
    /// call only when its callee is not a function name (D12: the name
    /// has its own entry) and may be an object of class type.
    fn written_oper_may_select(&self, x: ExprId, close: Loc, operands: &[ExprId]) -> bool {
        if operands.iter().any(|&o| self.contains_errors(o)) {
            return false;
        }
        let ex = &self.ast.exprs;
        let n = ex.get(x);
        let assign = n.kind() == ExprKind::BinaryOperator && ex.binary_op(x) == BinaryOp::Assign;
        match operands {
            [] => false,
            [callee, ..] if close.is_valid() && n.kind() == ExprKind::CallExpr => {
                !self.written_names_function(*callee) && self.written_could_overload(*callee)
            }
            [obj, ..] if close.is_valid() || assign => self.written_could_overload(*obj),
            ops => ops.iter().any(|&o| self.written_could_overload(o)),
        }
    }

    /// True when the callee `c` of a call is a function name (D12 of
    /// spec/ast.md §18.13.6): an unresolved or dependent name, or a name
    /// or member access naming a function or an overload set.
    fn written_names_function(&self, c: ExprId) -> bool {
        let c = self.written_oper_peel(self.ignore_parens(c));
        let ex = &self.ast.exprs;
        match ex.get(c).kind() {
            ExprKind::UnresolvedLookupExpr
            | ExprKind::DependentScopeDeclRefExpr
            | ExprKind::CXXDependentScopeMemberExpr => true,
            ExprKind::DeclRefExpr | ExprKind::MemberExpr => {
                let d = ex.decl(c);
                self.member_overload(c).is_some()
                    || self.ast.decls.function(d).is_some()
                    || self.kind_of(d) == DeclKind::FunctionTemplate
            }
            _ => false,
        }
    }

    /// A `DEPENDENT` `operator` entry at `tok` for token `slot` of the
    /// dependent expression `x`: its pattern node, its `close`, its found
    /// set and the using-directives its elements were found through.
    fn push_written_dependent_oper(&mut self, x: ExprId, slot: u8, tok: Span, close: Loc) {
        let Some(id) = self.push_written_ref(
            tok,
            None,
            None,
            RefRole::Operator,
            RefQualifier::NONE,
            REF_DEPENDENT,
        ) else {
            return;
        };
        if close.is_valid() {
            self.ast.written_refs.set_close(id, close);
        }
        self.ast.written_refs.note_pattern_node(PatternNode::Oper(x, slot), id);
        self.note_written_oper_found(id, x);
    }

    /// The found set of the dependent operator entry `id` made from `x`
    /// (spec/ast.md §18.12.2): the non-member operator functions the
    /// unqualified lookup at the definition found
    /// (`Ast::dependent_oper_candidates`, §8.5, with the rewritten forms'
    /// names), open to argument-dependent lookup (`ADL`); none for an
    /// operator with member candidates only (`=`, `[]`, `()`, `->`).
    fn note_written_oper_found(&mut self, id: WrittenRefId, x: ExprId) {
        let Some(cands) = self.ast.dependent_oper_candidates(x) else { return };
        let cands = cands.to_vec();
        let elems: Vec<FoundDecl> = cands
            .iter()
            .map(|c| {
                let target = self.ast.strip_shadow(c.decl);
                let decl = self.ast.first(target);
                let found = if c.found != c.decl && c.found != target { c.found } else { decl };
                FoundDecl { decl, found }
            })
            .collect();
        let s = self.ast.written_refs.intern_found(&elems, RefLookup::Unqualified, FOUND_ADL);
        self.ast.written_refs.set_found(id, s);
        // The using-directive each namespace-scope element was found
        // through, for the rows that select it (§18.9.1): the scope walk
        // with the element's name, believed only when it finds it.
        if cands.is_empty() || !self.any_using_directive() {
            return;
        }
        let r = *self.ast.written_refs.get(id);
        for c in &cands {
            let target = self.ast.strip_shadow(c.decl);
            if !self.in_namespace_scope(target) {
                continue;
            }
            let name = self.ast.decls.get(target).name();
            let Some((found, p)) = self.written_walk(&r, name, crate::lookup::LookupKind::Ordinary)
            else {
                continue;
            };
            let key = self.ast.first(target);
            if !found.decls.iter().any(|&f| self.ast.first(self.ast.strip_shadow(f)) == key) {
                continue;
            }
            if let Some(u) = self.written_udir_of(key, name, &p) {
                self.ast.written_refs.note_pattern_udir(id, key, u);
            }
        }
    }

    /// The dependent operator entries made from the pattern node `p`
    /// (a binary fold has two).
    fn written_oper_entries(&self, p: ExprId) -> [Option<WrittenRefId>; 2] {
        let t = &self.ast.written_refs;
        let first = t.pattern_entry(PatternNode::Oper(p, 0));
        [first, first.and_then(|_| t.pattern_entry(PatternNode::Oper(p, 1)))]
    }

    /// The substituter rebuilt the pattern expression `pattern` as
    /// `rebuilt`: when `pattern` is a dependent operator's node, what its
    /// token selected in the current specialization is that entry's row
    /// (spec/ast.md §18.13.6). A fold-expression's rows are made per
    /// application ([`Self::note_written_spec_fold`]). Callers test
    /// `WrittenRefs::has_pattern_nodes` first.
    pub(crate) fn note_written_spec_oper(&mut self, pattern: ExprId, rebuilt: ExprId) {
        if rebuilt == pattern {
            return;
        }
        let kind = self.ast.exprs.get(pattern).kind();
        let arrow = match kind {
            ExprKind::BinaryOperator
            | ExprKind::CompoundAssignOperator
            | ExprKind::UnaryOperator
            | ExprKind::ArraySubscriptExpr
            | ExprKind::CallExpr
            | ExprKind::CXXOperatorCallExpr
            | ExprKind::CXXNewExpr
            | ExprKind::CXXDeleteExpr
            | ExprKind::DependentCoawaitExpr
            | ExprKind::CoawaitExpr => false,
            ExprKind::CXXDependentScopeMemberExpr | ExprKind::MemberExpr => true,
            _ => return,
        };
        let ws = self.written_oper_entries(pattern);
        let Some(w) = ws[0] else { return };
        let Some(keys) = self.written_row_keys(w, Some(PatternNode::Oper(pattern, 0))) else {
            return;
        };
        let picks = if arrow {
            let at = self.ast.written_refs.get(w).range.lo;
            self.written_arrow_sels(w, rebuilt, at)
        } else {
            self.written_oper_sels(w, rebuilt)
        };
        for (slot, w) in (0u8..).zip(ws) {
            if let Some(w) = w {
                self.push_written_oper_rows(w, slot, keys, pattern, rebuilt, &picks);
            }
        }
    }

    /// One application `r` of the expansion of the fold-expression
    /// `pattern` (or the fold it was rebuilt as, still over a pack): a
    /// row of each of the fold's operator entries, one per distinct
    /// resolution (rows are kept once, §18.9.1).
    pub(crate) fn note_written_spec_fold(&mut self, pattern: ExprId, r: ExprId) {
        let ws = self.written_oper_entries(pattern);
        let Some(w0) = ws[0] else { return };
        let Some(keys) = self.written_row_keys(w0, Some(PatternNode::Oper(pattern, 0))) else {
            return;
        };
        let picks = if self.ast.exprs.get(r).kind() == ExprKind::CXXFoldExpr {
            vec![OperSel::Dependent]
        } else {
            self.written_oper_sels(w0, r)
        };
        for (slot, w) in (0u8..).zip(ws) {
            if let Some(w) = w {
                self.push_written_oper_rows(w, slot, keys, pattern, r, &picks);
            }
        }
    }

    /// The expansion of the fold-expression `pattern` applied no operator
    /// ([temp.variadic]/13: an empty pack's `&&`, `||`, `,`, a single
    /// element, an init alone): the token was reached and selected
    /// nothing, a `SPEC_BUILTIN` row.
    pub(crate) fn note_written_spec_fold_none(&mut self, pattern: ExprId) {
        let ws = self.written_oper_entries(pattern);
        let Some(w0) = ws[0] else { return };
        let Some(keys) = self.written_row_keys(w0, Some(PatternNode::Oper(pattern, 0))) else {
            return;
        };
        for (slot, w) in (0u8..).zip(ws) {
            if let Some(w) = w {
                self.push_written_oper_rows(
                    w,
                    slot,
                    keys,
                    pattern,
                    pattern,
                    &[OperSel::Builtin(None)],
                );
            }
        }
    }

    /// The rows of entry `w` (token `slot` of `pattern`'s node) for
    /// `picks` under `keys`; a still-dependent `rebuilt` becomes a further
    /// pattern node of the entry (§18.9.4), and a failure inside a SFINAE
    /// context records nothing (§18.9.3) and fails a key scope's
    /// substitution (§18.14.3).
    fn push_written_oper_rows(
        &mut self,
        w: WrittenRefId,
        slot: u8,
        keys: RowKeys,
        pattern: ExprId,
        rebuilt: ExprId,
        picks: &[OperSel],
    ) {
        let node = PatternNode::Oper(pattern, slot);
        let keys = if slot == 0 { keys } else { self.rekey(keys, w, node) };
        for sel in picks {
            match sel {
                OperSel::Fn((d, f), flags, how, naming, adl) => {
                    self.push_written_spec_with(
                        w,
                        keys,
                        Some(*d),
                        *f,
                        *flags,
                        *how,
                        *naming,
                        adl.clone(),
                    );
                }
                OperSel::Builtin(adl) => {
                    self.push_written_spec_with(
                        w,
                        keys,
                        None,
                        None,
                        SPEC_BUILTIN,
                        None,
                        None,
                        adl.clone(),
                    );
                }
                OperSel::Unresolved if self.inst.stack.is_sfinae() => {
                    if let Some(k) = keys.key {
                        self.key_scope_poison(k);
                    }
                }
                OperSel::Unresolved => {
                    self.push_written_spec_with(w, keys, None, None, 0, None, None, None);
                }
                OperSel::Dependent => {
                    let new = PatternNode::Oper(rebuilt, slot);
                    self.ast.written_refs.note_pattern_node(new, w);
                    self.note_keyed_still_dependent(w, keys.key, node, new);
                }
            }
        }
    }

    /// What the rebuilt operator expression `r` selected
    /// (spec/ast.md §18.13.6): the function of its operator call, the
    /// candidates of a rewritten comparison (§18.13.3), a surrogate call's
    /// conversion function; a built-in operator, or a call of a function
    /// pointer, selects nothing (carrying the associated ADL set when
    /// overload resolution ran ADL on a class or enumeration operand,
    /// WP-3701).
    fn written_oper_sels(&mut self, w: WrittenRefId, r: ExprId) -> Vec<OperSel> {
        let x = self.written_oper_peel(r);
        let n = *self.ast.exprs.get(x);
        if self.is_err_expr(x) {
            return vec![OperSel::Unresolved];
        }
        if n.is_type_dependent() {
            return vec![OperSel::Dependent];
        }
        match n.kind() {
            ExprKind::CXXOperatorCallExpr => vec![self.written_call_sel(w, x, 0, None)],
            ExprKind::CXXRewrittenBinaryOperator => {
                match self.written_rewritten_calls(x, n.loc()) {
                    Some((first, second, reversed, written)) => {
                        let flags = REF_REWRITTEN | if reversed { REF_REVERSED } else { 0 };
                        let mut v = vec![self.written_call_sel(w, first, flags, Some(&written))];
                        if let Some(c) = second {
                            v.push(self.written_call_sel(w, c, REF_REWRITTEN, None));
                        }
                        v
                    }
                    None => vec![OperSel::Builtin(None)],
                }
            }
            ExprKind::CallExpr => match self.written_surrogate(self.ast.exprs.callee(x)) {
                Some((d, f, naming)) => {
                    vec![OperSel::Fn((d, f), 0, Some(RefLookup::Member), naming, None)]
                }
                None => vec![OperSel::Builtin(None)],
            },
            // Stage 6c: the allocation or deallocation function (the
            // global replaceable ones count as functions); a still
            // dependent one stands for the entry.
            ExprKind::CXXNewExpr | ExprKind::CXXDeleteExpr => match self.written_alloc_fn(x) {
                Some(f) => {
                    let naming = if self.kind_of(f) == DeclKind::CXXMethod {
                        self.written_alloc_class(x)
                    } else {
                        None
                    };
                    vec![OperSel::Fn(
                        self.written_fn_found(f),
                        0,
                        Some(RefLookup::Qualified),
                        naming,
                        None,
                    )]
                }
                _ if self.written_alloc_dependent(x) => vec![OperSel::Dependent],
                _ => vec![OperSel::Builtin(None)],
            },
            ExprKind::DependentCoawaitExpr => vec![OperSel::Dependent],
            ExprKind::CoawaitExpr => match self.written_coawait_call(x) {
                Some(c) => vec![self.written_call_sel(w, c, 0, None)],
                None => vec![OperSel::Builtin(None)],
            },
            _ => vec![OperSel::Builtin(self.written_builtin_spec_adl(x))],
        }
    }

    /// The associated set for a dependent operator specialization that
    /// selected a built-in candidate on a class or enumeration operand
    /// (`[over.match.oper]/1-2`, WP-3701); `None` when no overloadable
    /// operand is present or the operator has no non-member lookup.
    fn written_builtin_spec_adl(&mut self, x: ExprId) -> Option<AdlSet> {
        if self.is_c() {
            return None;
        }
        let n = *self.ast.exprs.get(x);
        let ops: Vec<ExprId> = match n.kind() {
            ExprKind::BinaryOperator => {
                let bop = self.ast.exprs.binary_op(x);
                if matches!(bop, BinaryOp::PtrMemD | BinaryOp::Assign) {
                    return None;
                }
                let (l, r) = (ExprId::new(n.a()), ExprId::new(n.b()));
                vec![self.written_oper_unconverted(l), self.written_oper_unconverted(r)]
            }
            ExprKind::CompoundAssignOperator => {
                let (l, r) = (ExprId::new(n.a()), ExprId::new(n.b()));
                vec![self.written_oper_unconverted(l), self.written_oper_unconverted(r)]
            }
            ExprKind::UnaryOperator => {
                let uop = self.ast.exprs.unary_op(x);
                if crate::overload::oper::unary_oper(uop).is_none()
                    || (uop == UnaryOp::AddrOf
                        && self.ast.types.canonical_kind(self.ety(x))
                            == kiln_ast::types::TypeKind::MemberPointer)
                {
                    return None;
                }
                vec![self.written_oper_unconverted(self.ast.exprs.operand(x))]
            }
            _ => return None,
        };
        if !ops.iter().any(|&o| self.overloadable_operand(o)) {
            return None;
        }
        let a = self.adl_associated_set(&ops, n.loc());
        Some(AdlSet::new(&a.namespaces, &a.classes, &a.enums))
    }

    /// The selection of the resolved operator call `c`, with `flags`,
    /// re-running ADL against entry `w`'s definition-context found set for
    /// non-member calls (WP-2920).
    fn written_call_sel(
        &mut self,
        w: WrittenRefId,
        c: ExprId,
        flags: u8,
        adl_override: Option<&[ExprId]>,
    ) -> OperSel {
        let n = *self.ast.exprs.get(c);
        let Some(t) = self.written_callee(self.ast.exprs.callee(c)) else {
            return OperSel::Unresolved;
        };
        if matches!(self.kind_of(t.0), DeclKind::CXXMethod | DeclKind::CXXConversion) {
            let naming = self
                .ast
                .lists
                .iter::<ExprId>(self.ast.exprs.call_args(c))
                .next()
                .and_then(|a| self.written_oper_object_class(a));
            return OperSel::Fn(t, flags, Some(RefLookup::Member), naming, None);
        }
        let is_adl = n.has(ExprFlags::ADL);
        let mut how = is_adl.then_some(RefLookup::Adl);
        let mut adl_set = None;
        if !self.is_c() {
            let op = self.ast.exprs.operator_call_op(c);
            let dn = DeclName::operator(op);
            let mut fns: Vec<(DeclId, DeclId)> = if is_adl {
                Vec::new()
            } else {
                self.ast
                    .written_refs
                    .found_at_definition(w)
                    .map(|set| {
                        set.decls
                            .iter()
                            .filter(|e| self.ast.decls.get(e.decl).name() == dn)
                            .map(|e| (e.decl, e.found))
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let owned;
            let args: &[ExprId] = if let Some(a) = adl_override {
                a
            } else {
                owned = self.written_call_unconverted_args(c);
                &owned
            };
            if let Some(facts) = self.adl_lookup_explained(dn, args, n.loc(), &mut fns, true)
                && let Some((refined, set)) = self.written_call_lookup(Some(t.0), t.1, &facts)
            {
                if !is_adl || refined == Some(RefLookup::Adl) {
                    how = refined.or(how);
                }
                adl_set = Some(set);
            }
        }
        OperSel::Fn(t, flags, how, None, adl_set)
    }

    /// The pre-derived-to-base object class of a member operator's object
    /// operand `e` ([class.access.base]/5).
    fn written_oper_object_class(&self, mut e: ExprId) -> Option<DeclId> {
        let ex = &self.ast.exprs;
        for _ in 0..32 {
            match ex.get(e).kind() {
                ExprKind::ImplicitCastExpr
                    if matches!(
                        ex.cast_kind(e),
                        CastKind::DerivedToBase
                            | CastKind::UncheckedDerivedToBase
                            | CastKind::NoOp
                            | CastKind::LValueToRValue
                    ) =>
                {
                    e = ex.operand(e);
                }
                ExprKind::MaterializeTemporaryExpr
                | ExprKind::CXXBindTemporaryExpr
                | ExprKind::ParenExpr
                | ExprKind::ConstantExpr
                | ExprKind::ExprWithCleanups => e = ex.operand(e),
                _ => break,
            }
        }
        self.ast.types.as_record(self.ety(e))
    }

    /// The class scope in which a class-member `operator new` or
    /// `operator delete` of `x` was looked up ([expr.new]/12, [expr.delete]/9).
    fn written_alloc_class(&self, x: ExprId) -> Option<DeclId> {
        let ty = match self.ast.exprs.get(x).kind() {
            ExprKind::CXXNewExpr => self.ast.exprs.new_extra(x).allocated_ty,
            ExprKind::CXXDeleteExpr => {
                self.ast.types.pointee(self.ety(self.ast.exprs.delete_operand(x)))?
            }
            _ => return None,
        };
        self.ast.types.as_record(self.base_element(ty))
    }

    /// What the `->` at `at` of the rebuilt member access `r` applied:
    /// each `operator->` in application order ([over.ref]/1); a built-in
    /// `->` (a pointer) nothing; a class object with no `operator->`
    /// (diagnosed) a failure.
    fn written_arrow_sels(&mut self, w: WrittenRefId, r: ExprId, at: Loc) -> Vec<OperSel> {
        let x = self.written_oper_peel(r);
        let mut chain = Vec::new();
        let mut cur = self.written_arrow_outermost(x, at);
        while let Some(c) = cur.map(|c| self.written_oper_peel(c))
            && self.ast.exprs.get(c).kind() == ExprKind::CXXOperatorCallExpr
            && self.ast.exprs.operator_call_op(c) == OO::Arrow
            && self.ast.exprs.get(c).loc() == at
        {
            chain.push(c);
            cur = self.ast.lists.iter::<ExprId>(self.ast.exprs.call_args(c)).next();
        }
        if !chain.is_empty() {
            return chain.iter().rev().map(|&c| self.written_call_sel(w, c, 0, None)).collect();
        }
        if self.is_err_expr(x) {
            // The member access failed: whether its `->` was built in is
            // not known.
            return Vec::new();
        }
        if self.ast.exprs.get(x).is_type_dependent()
            && matches!(self.ast.exprs.get(x).kind(), ExprKind::CXXDependentScopeMemberExpr)
        {
            return vec![OperSel::Dependent];
        }
        let mut kids = Vec::new();
        self.ast.exprs.children_into(x, &self.ast.lists, &mut kids);
        match kids.first() {
            Some(&b) if self.ast.types.as_record(self.ety(b)).is_some() => {
                vec![OperSel::Unresolved]
            }
            _ => vec![OperSel::Builtin(None)],
        }
    }

    /// Peels implicit conversions (including a user-defined conversion
    /// call `c.operator T()`) off an operator operand so ADL sees the
    /// pre-conversion class or enumeration expression.
    fn written_oper_unconverted(&self, mut e: ExprId) -> ExprId {
        let ex = &self.ast.exprs;
        for _ in 0..32 {
            match ex.get(e).kind() {
                ExprKind::ImplicitCastExpr
                    if ex.cast_kind(e) == CastKind::UserDefinedConversion =>
                {
                    let call = self.written_oper_peel(ex.operand(e));
                    if ex.get(call).kind() == ExprKind::CXXMemberCallExpr {
                        let m = self.written_oper_peel(ex.callee(call));
                        if ex.get(m).kind() == ExprKind::MemberExpr {
                            e = ex.operand(m);
                            continue;
                        }
                    }
                    e = ex.operand(e);
                }
                ExprKind::ImplicitCastExpr
                | ExprKind::MaterializeTemporaryExpr
                | ExprKind::CXXBindTemporaryExpr
                | ExprKind::ConstantExpr
                | ExprKind::ExprWithCleanups => e = ex.operand(e),
                _ => return e,
            }
        }
        e
    }

    /// The arguments of `CXXOperatorCallExpr` `c` with implicit conversions
    /// peeled, for re-running ADL at the recording site.
    fn written_call_unconverted_args(&self, c: ExprId) -> Vec<ExprId> {
        self.ast
            .lists
            .iter::<ExprId>(self.ast.exprs.call_args(c))
            .map(|a| self.written_oper_unconverted(a))
            .collect()
    }

    /// `e` without the implicit nodes sema puts around an operator call
    /// or a user-defined literal's call (a temporary's binding and
    /// materialization, an immediate invocation's `ConstantExpr`,
    /// conversions, cleanups).
    pub(crate) fn written_oper_peel(&self, mut e: ExprId) -> ExprId {
        let ex = &self.ast.exprs;
        for _ in 0..64 {
            match ex.get(e).kind() {
                ExprKind::ImplicitCastExpr
                | ExprKind::MaterializeTemporaryExpr
                | ExprKind::CXXBindTemporaryExpr
                | ExprKind::ConstantExpr
                | ExprKind::ExprWithCleanups => e = ex.operand(e),
                _ => return e,
            }
        }
        e
    }

    /// The function a resolved callee `callee` names and what lookup
    /// found when it differs (the using-shadow, or the template of a
    /// specialization); `None` for an unresolved or dependent callee.
    fn written_callee(&self, callee: ExprId) -> Option<(DeclId, Option<DeclId>)> {
        let c = self.written_oper_peel(callee);
        if !matches!(self.ast.exprs.get(c).kind(), ExprKind::DeclRefExpr | ExprKind::MemberExpr) {
            return None;
        }
        let d = self.ast.exprs.decl(c);
        let found = self
            .ast
            .exprs
            .decl_ref_extra(c)
            .and_then(|x| x.found_decl)
            .or_else(|| self.ast.spec_info(d).map(|(_, i)| i.template))
            .filter(|&f| f != d);
        Some((d, found))
    }

    /// The conversion function of a surrogate call whose callee is
    /// `callee`: the object under a user-defined conversion (an implicit
    /// cast, so not a written `c.operator FP()(x)`) by its `operator FP()`,
    /// together with the object's pre-conversion class; `None` for any
    /// other call.
    fn written_surrogate(
        &self,
        callee: ExprId,
    ) -> Option<(DeclId, Option<DeclId>, Option<DeclId>)> {
        let ex = &self.ast.exprs;
        let mut c = callee;
        while ex.get(c).kind() == ExprKind::ImplicitCastExpr
            && ex.cast_kind(c) != CastKind::UserDefinedConversion
        {
            c = ex.operand(c);
        }
        if ex.get(c).kind() != ExprKind::ImplicitCastExpr {
            return None;
        }
        let call = self.written_oper_peel(ex.operand(c));
        if ex.get(call).kind() != ExprKind::CXXMemberCallExpr {
            return None;
        }
        let m = self.written_oper_peel(ex.callee(call));
        if ex.get(m).kind() != ExprKind::MemberExpr {
            return None;
        }
        let d = ex.decl(m);
        if self.kind_of(d) != DeclKind::CXXConversion {
            return None;
        }
        let (decl, found) = self.written_callee(m)?;
        let naming = self.written_oper_object_class(ex.operand(m));
        Some((decl, found, naming))
    }

    /// The entry of the resolved operator call `call` at `tok`, with
    /// full lookup provenance (`how=adl` / `how=both` / `unqualified`, and
    /// the associated ADL set for non-member candidates; WP-2920).
    fn push_written_oper_call(
        &mut self,
        call: ExprId,
        tok: Span,
        close: Loc,
        flags: u8,
        adl_args: &[ExprId],
    ) {
        let n = *self.ast.exprs.get(call);
        // A dependent operator (an unresolved callee) is stage 6b's.
        if n.is_type_dependent() {
            return;
        }
        let Some(target) = self.written_callee(self.ast.exprs.callee(call)) else { return };
        let adl = n.has(ExprFlags::ADL);
        let Some(id) = self.push_written_oper(tok, target, close, flags) else { return };
        if matches!(self.kind_of(target.0), DeclKind::CXXMethod | DeclKind::CXXConversion) {
            return;
        }
        if !self.is_c() {
            let op = self.ast.exprs.operator_call_op(call);
            let dn = DeclName::operator(op);
            let mut fns: Vec<(DeclId, DeclId)> = if adl {
                Vec::new()
            } else {
                let found = self.lookup_operator_name(dn);
                found.decls.iter().copied().zip(found.found.iter().copied()).collect()
            };
            let facts = self.adl_lookup_explained(dn, adl_args, n.loc(), &mut fns, true);
            self.note_written_call_lookup(id, facts);
        }
        if adl && self.ast.written_refs.get(id).how != RefLookup::Adl {
            // Found only by argument-dependent lookup (clang's
            // `IsADLCandidate`): no using-directive applies.
            self.ast.written_refs.set_how(id, RefLookup::Adl);
            self.ast.written_refs.set_using_directive(id, None);
        }
    }

    /// An `operator` entry for a built-in candidate selected by operator
    /// overload resolution on a class or enumeration operand
    /// ([over.match.oper]/1-2, [over.built]; spec/ast.md §18.13, WP-3701):
    /// no entity, flag `REF_BUILTIN`, `how=builtin`, and the associated ADL
    /// set when non-member ADL ran. Operators with no class/enum operand,
    /// non-overloadable operators (`?:`, `.*`), `&C::m`, and normal function
    /// calls get no entry.
    fn note_written_oper_builtin(
        &mut self,
        x: ExprId,
        tok: Span,
        at: Loc,
        close: Loc,
        operands: &[ExprId],
    ) {
        if self.is_c()
            || operands
                .iter()
                .any(|&o| self.ast.exprs.get(o).is_type_dependent() || self.contains_errors(o))
        {
            return;
        }
        let n = *self.ast.exprs.get(x);
        let member_only = match n.kind() {
            ExprKind::BinaryOperator if n.loc() == at => {
                let bop = self.ast.exprs.binary_op(x);
                if matches!(bop, BinaryOp::PtrMemD | BinaryOp::Assign)
                    || !operands.iter().any(|&o| self.overloadable_operand(o))
                {
                    return;
                }
                false
            }
            ExprKind::CompoundAssignOperator if n.loc() == at => {
                if !operands.iter().any(|&o| self.overloadable_operand(o)) {
                    return;
                }
                false
            }
            ExprKind::UnaryOperator if n.loc() == at => {
                let uop = self.ast.exprs.unary_op(x);
                if crate::overload::oper::unary_oper(uop).is_none()
                    || (uop == UnaryOp::AddrOf
                        && self.ast.types.canonical_kind(self.ety(x))
                            == kiln_ast::types::TypeKind::MemberPointer)
                    || !operands.iter().any(|&o| self.overloadable_operand(o))
                {
                    return;
                }
                false
            }
            ExprKind::ArraySubscriptExpr => {
                let has_cls = operands
                    .first()
                    .is_some_and(|&a0| self.ast.types.as_record(self.ety(a0)).is_some())
                    || (operands.len() == 2
                        && self.ast.types.as_record(self.ety(operands[1])).is_some());
                if !has_cls {
                    return;
                }
                true
            }
            _ => return,
        };
        let Some(id) = self.push_written_ref(
            tok,
            None,
            None,
            RefRole::Operator,
            RefQualifier::NONE,
            REF_BUILTIN,
        ) else {
            return;
        };
        self.ast.written_refs.set_how(id, RefLookup::Builtin);
        if close.is_valid() {
            self.ast.written_refs.set_close(id, close);
        }
        if !member_only {
            let a = self.adl_associated_set(operands, at);
            self.ast.written_refs.set_adl(id, AdlSet::new(&a.namespaces, &a.classes, &a.enums));
        }
    }

    /// An `operator` entry at `tok` naming `entity` (found as `found`):
    /// `how` is `member` for a member function (a member candidate,
    /// [over.match.oper]/3.1, a call operator, `operator->`, a surrogate's
    /// conversion function), else as the scope walk says (§18.7).
    fn push_written_oper(
        &mut self,
        tok: Span,
        (entity, found): (DeclId, Option<DeclId>),
        close: Loc,
        flags: u8,
    ) -> Option<WrittenRefId> {
        let id = self.push_written_ref(
            tok,
            Some(entity),
            found,
            RefRole::Operator,
            RefQualifier::NONE,
            flags,
        )?;
        if matches!(self.kind_of(entity), DeclKind::CXXMethod | DeclKind::CXXConversion) {
            self.ast.written_refs.set_how(id, RefLookup::Member);
            self.ast.written_refs.set_using_directive(id, None);
        }
        if close.is_valid() {
            self.ast.written_refs.set_close(id, close);
        }
        Some(id)
    }

    /// An `UNRESOLVED` `operator` entry at `tok`.
    fn push_written_oper_unresolved(&mut self, tok: Span, close: Loc) {
        let id = self.push_written_ref(
            tok,
            None,
            None,
            RefRole::Operator,
            RefQualifier::NONE,
            REF_UNRESOLVED,
        );
        if let Some(id) = id
            && close.is_valid()
        {
            self.ast.written_refs.set_close(id, close);
        }
    }

    /// An operator expression that failed: `UNRESOLVED` when overload
    /// resolution ran on it, i.e. an operand of class or enumeration type
    /// ([over.match.oper]/1; for `[]` and `()`, the object), none of
    /// them erroneous or dependent.
    fn note_written_oper_failure(&mut self, tok: Span, close: Loc, operands: &[ExprId]) {
        let mut overloadable = false;
        for (i, &o) in operands.iter().enumerate() {
            if self.ast.exprs.get(o).is_type_dependent() || self.contains_errors(o) {
                return;
            }
            // `[]` and `()` resolve only on a class object ([over.sub],
            // [over.call]): their arguments do not decide.
            if i == 0 || !close.is_valid() {
                overloadable |= self.overloadable_operand(o);
            }
        }
        if overloadable {
            self.push_written_oper_unresolved(tok, close);
        }
    }

    /// The entries of the rewritten comparison `x` at `tok`
    /// ([over.match.oper]/3.4, /8, /9): the selected `operator==` or
    /// `operator<=>` (`rewritten`, and `reversed` for the reversed
    /// synthesized candidate), then, when the comparison of a `<=>`
    /// result with zero is itself an overloaded operator, that function
    /// (`rewritten`), in application order (spec/ast.md §18.13.3).
    fn note_written_rewritten(&mut self, x: ExprId, tok: Span) {
        let Some((first, second, reversed, written)) = self.written_rewritten_calls(x, tok.lo)
        else {
            return;
        };
        let flags = REF_REWRITTEN | if reversed { REF_REVERSED } else { 0 };
        self.push_written_oper_call(first, tok, Loc::INVALID, flags, &written);
        if let Some(c) = second {
            let adl_args = self.written_call_unconverted_args(c);
            self.push_written_oper_call(c, tok, Loc::INVALID, REF_REWRITTEN, &adl_args);
        }
    }

    /// The operator calls of the rewritten comparison `x` whose operator
    /// is at `at`: the selected `operator==` or `operator<=>`, the
    /// overloaded comparison of a `<=>` result with zero if any, whether
    /// the first is the reversed synthesized candidate, and the written
    /// operands of `x` (unconverted, for ADL).
    fn written_rewritten_calls(
        &self,
        x: ExprId,
        at: Loc,
    ) -> Option<(ExprId, Option<ExprId>, bool, Vec<ExprId>)> {
        let ex = &self.ast.exprs;
        let (_, list) = ex.rewritten_written(x);
        let written: Vec<ExprId> = self.ast.lists.iter::<ExprId>(list).collect();
        let mut s = self.written_oper_peel(ex.rewritten_semantic(x));
        // `x != y` is `!(x == y)`, its `!` built-in (the `==` returns
        // `bool`, [over.match.oper]/9).
        if ex.get(s).kind() == ExprKind::UnaryOperator && ex.unary_op(s) == UnaryOp::LNot {
            s = self.written_oper_peel(ex.operand(s));
        }
        // The `<=>` call at the token among the operands of `s` (the
        // comparison with zero), else `s` itself.
        let mut kids = Vec::new();
        ex.children_into(s, &self.ast.lists, &mut kids);
        let inner = kids.into_iter().map(|k| self.written_oper_peel(k)).find(|&k| {
            let n = ex.get(k);
            n.kind() == ExprKind::CXXOperatorCallExpr
                && n.loc() == at
                && ex.operator_call_op(k) == OO::Spaceship
        });
        let (first, second) = match inner {
            Some(i) if ex.get(s).kind() == ExprKind::CXXOperatorCallExpr => (i, Some(s)),
            Some(i) => (i, None),
            None if ex.get(s).kind() == ExprKind::CXXOperatorCallExpr => (s, None),
            None => return None,
        };
        // Reversed: the selected function's first argument is the written
        // right operand.
        let arg0 = self.ast.lists.iter::<ExprId>(ex.call_args(first)).next();
        let reversed = match (arg0, written.as_slice()) {
            (Some(a), [l, r]) => {
                let (a, l, r) = (self.begin_loc(a), self.begin_loc(*l), self.begin_loc(*r));
                a == r && a != l
            }
            _ => false,
        };
        Some((first, second, reversed, written))
    }
}
