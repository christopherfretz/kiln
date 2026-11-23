//! Stage 6 of the written-reference table (spec/ast.md §18.13, WP-2820):
//! the operator token of an operator expression whose overload resolution
//! selected a function, and the ud-suffix of a user-defined literal. The
//! parser calls these with the expression it just built at the token; the
//! facts are read from that expression (no sema side state), and nothing
//! is recorded during instantiation or when recording is off (one branch).

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{CastKind, ExprFlags, ExprKind, UnaryOp};
use kiln_ast::names::OverloadedOperator as OO;
use kiln_ast::written_refs::{
    REF_REVERSED, REF_REWRITTEN, REF_UNRESOLVED, RefLookup, RefQualifier, RefRole, WrittenRefId,
};
use kiln_ast::{DeclId, ExprId};
use kiln_source::{Loc, Span};

use crate::Sema;

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
    /// operator in a template pattern records nothing yet (stage 6b).
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
        match n.kind() {
            ExprKind::CXXOperatorCallExpr if n.loc() == at => {
                self.push_written_oper_call(x, tok, close, 0);
            }
            ExprKind::CXXRewrittenBinaryOperator if n.loc() == at => {
                self.note_written_rewritten(x, tok);
            }
            ExprKind::CallExpr => {
                // A surrogate call function ([over.call.object]/2): the
                // callee is the object implicitly converted to a pointer
                // to function.
                if let Some(conv) = self.written_surrogate(self.ast.exprs.callee(x)) {
                    self.push_written_oper(tok, conv, close, 0);
                }
            }
            _ if self.is_err_expr(x) => self.note_written_oper_failure(tok, close, operands),
            _ => {}
        }
    }

    /// Records the `->` at `tok` of the class member access `e` whose
    /// object expression was `base`: one entry per `operator->` the
    /// chain applied, in application order ([over.ref]/1, spec/ast.md
    /// §18.13); `UNRESOLVED` when `base` has class type and no
    /// `operator->` was applied (diagnosed); nothing for a built-in `->`.
    pub fn note_written_arrow(&mut self, e: ExprId, base: ExprId, tok: Span) {
        if !self.records_written_refs() || !self.inst.stack.is_empty() {
            return;
        }
        if self.ast.exprs.get(base).is_type_dependent()
            || self.ast.types.as_record(self.ety(base)).is_none()
            || self.contains_errors(base)
        {
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
            self.push_written_oper_call(c, tok, Loc::INVALID, 0);
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
    /// cast, so not a written `c.operator FP()(x)`) by its `operator FP()`;
    /// `None` for any other call.
    fn written_surrogate(&self, callee: ExprId) -> Option<(DeclId, Option<DeclId>)> {
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
        (self.kind_of(d) == DeclKind::CXXConversion).then(|| self.written_callee(m)).flatten()
    }

    /// The entry of the resolved operator call `call` at `tok`.
    fn push_written_oper_call(&mut self, call: ExprId, tok: Span, close: Loc, flags: u8) {
        let n = *self.ast.exprs.get(call);
        // A dependent operator (an unresolved callee) is stage 6b's.
        if n.is_type_dependent() {
            return;
        }
        let Some(target) = self.written_callee(self.ast.exprs.callee(call)) else { return };
        let adl = n.has(ExprFlags::ADL);
        let id = self.push_written_oper(tok, target, close, flags);
        if adl && let Some(id) = id {
            // Found only by argument-dependent lookup (clang's
            // `IsADLCandidate`): no using-directive applies.
            self.ast.written_refs.set_how(id, RefLookup::Adl);
            self.ast.written_refs.set_using_directive(id, None);
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
                && n.loc() == tok.lo
                && ex.operator_call_op(k) == OO::Spaceship
        });
        let (first, second) = match inner {
            Some(i) if ex.get(s).kind() == ExprKind::CXXOperatorCallExpr => (i, Some(s)),
            Some(i) => (i, None),
            None if ex.get(s).kind() == ExprKind::CXXOperatorCallExpr => (s, None),
            None => return,
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
        let flags = REF_REWRITTEN | if reversed { REF_REVERSED } else { 0 };
        self.push_written_oper_call(first, tok, Loc::INVALID, flags);
        if let Some(c) = second {
            self.push_written_oper_call(c, tok, Loc::INVALID, REF_REWRITTEN);
        }
    }
}
