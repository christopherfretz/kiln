//! Calls of overloaded member functions ([over.match.call]/2,
//! [over.match.funcs]/4-5; spec/sema-overload.md §2.3): `b.f(…)`,
//! `p->f(…)` and the implicit `this->f(…)` of a member name, where `f`
//! names more than one member function. The member access is a
//! `MemberExpr` of type [`QualType::OVERLOAD`] (kiln's
//! `UnresolvedMemberExpr`) whose set [`OvlMember`] records; the call
//! resolves it against its arguments and the object argument, and the
//! winner becomes the ordinary member access the call path binds.

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::{ExprKind, ValueCategory};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_source::Loc;

use super::{ArgInfo, CandidateKind, IcsOptions, OvlMember, SetKind};
use crate::Sema;
use crate::class::MemberRef;
use crate::init::InitList;

impl Sema<'_, '_> {
    /// Whether the members `decls` form a member function overload set
    /// (functions and function templates, at least one of them a member).
    pub(crate) fn is_member_overload_set(&self, decls: &[DeclId]) -> bool {
        decls.len() > 1
            && decls.iter().all(|&d| {
                let k = self.kind_of(self.templated_or_self(d));
                k.is_function() && k != DeclKind::Function
            })
    }

    fn templated_or_self(&self, d: DeclId) -> DeclId {
        if self.kind_of(d) == DeclKind::FunctionTemplate {
            self.pattern_of(d).unwrap_or(d)
        } else {
            d
        }
    }

    /// The overloaded member access of `m` on the object `base` (a pointer
    /// for `m.arrow`) at `loc`: a `MemberExpr` of overloaded type naming
    /// the first member.
    pub(crate) fn build_member_overload(&mut self, m: OvlMember, loc: Loc) -> ExprId {
        let e = self.builder().member(m.base, m.decls[0], QualType::OVERLOAD, m.arrow, loc, None);
        self.ovl.members.insert(e, m);
        e
    }

    /// The overloaded member access `e` names (through parentheses).
    pub(crate) fn member_overload(&self, e: ExprId) -> Option<ExprId> {
        if self.ast.exprs.get(e).ty() != QualType::OVERLOAD {
            return None;
        }
        let x = self.ignore_parens(e);
        (self.ast.exprs.get(x).kind() == ExprKind::MemberExpr && self.ovl.members.contains_key(&x))
            .then_some(x)
    }

    /// The member functions of the overloaded member access `x`.
    pub(crate) fn member_overload_decls(&self, x: ExprId) -> &[DeclId] {
        self.ovl.members.get(&x).map_or(&[], |m| &m.decls)
    }

    /// The qualified overloaded member name `x` (an implicit `this->C::f`)
    /// as the `UnresolvedLookupExpr` `C::f` it was written as.
    pub(crate) fn member_overload_as_lookup(&mut self, x: ExprId) -> Option<ExprId> {
        let nns = self.ovl.members.get(&x)?.nns?;
        let m = self.ovl.members.remove(&x)?;
        let loc = self.ast.exprs.get(x).loc();
        Some(self.build_overload_ref(m.name, &m.decls, &m.found, Some(m.naming), false, Some(nns), loc))
    }

    /// Resolves the call of the overloaded member access `callee`
    /// (possibly parenthesized) with `args`: the member access naming the
    /// best viable function, or `None` after diagnosing.
    pub(crate) fn resolve_member_call(
        &mut self,
        callee: ExprId,
        args: &[ExprId],
        braced: &[Option<InitList>],
    ) -> Option<ExprId> {
        let x = self.member_overload(callee)?;
        let m = self.ovl.members.remove(&x)?;
        let loc = self.ast.exprs.get(x).loc();
        let name = m.name;
        let quals = self.ast.types.effective_quals(m.obj_ty);
        let vk = if m.arrow {
            ValueCategory::LValue
        } else {
            self.ast.exprs.get(m.base).value_category()
        };
        let mut set = self.begin_set(SetKind::Normal, loc);
        let mark = self.push_call_args(&mut set, args, braced);
        let obj = self.ast.types.qualify(self.tag_type(m.def), quals);
        set.object = Some(ArgInfo::of_type(obj, vk));
        for (&d, &f) in m.decls.iter().zip(&m.found) {
            if self.ast.decls.get(d).has(DeclFlags::INVALID) {
                continue;
            }
            if self.kind_of(d) == DeclKind::FunctionTemplate {
                set.skipped_templates = true;
                continue;
            }
            if self.ast.decls.cold.requires_clause.contains(d) {
                set.uncertain = true;
            }
            self.add_candidate(&mut set, d, f, CandidateKind::Method, IcsOptions::NONE);
        }
        let out = self.best_viable(&mut set);
        self.ovl.lists.truncate(mark);
        let won = self.call_winner(&set, out, name, loc, loc, true);
        let won = won.map(|b| {
            let c = set.cand(b);
            (c.func(), c.found)
        });
        self.end_set(set);
        let (d, f) = won?;
        self.check_member_access(f, m.naming, Some(m.def), loc);
        let shadow = (f != d).then_some(f);
        if m.implicit && self.is_static_method(d) {
            let e = self.build_decl_ref_qualified(d, loc, shadow, m.nns);
            self.note_function_name_use(d, e, loc, m.nns.is_none());
            return Some(e);
        }
        let e = self.member_of_object(m.base, m.obj_ty, m.def, m.arrow, d, shadow, loc);
        if m.nns.is_some() && !self.is_err_expr(e) {
            self.member_refs.insert(e, MemberRef { decl: d, error: false, qualified: true });
        }
        Some(e)
    }
}
