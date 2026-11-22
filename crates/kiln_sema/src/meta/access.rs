//! The access queries ([meta.reflection.access.context],
//! [meta.reflection.access.queries]; spec/reflection-members.md §3;
//! WP-478): `access_context::current()`, `current_function` and its
//! siblings, `is_accessible` and the `has_inaccessible_*` queries, on
//! sema's access check ([`Sema::member_accessible_in`]).

use kiln_ast::decl::DeclKind;
use kiln_ast::reflect::Reflectee;
use kiln_ast::{DeclId, ReflectionId};
use kiln_oven::{
    AccessOp, MemberOp, MetaCtx, MetaError, MetaOut, Oven, info_arg, member_kind as mk,
};
use kiln_source::Loc;

use super::fail;
use crate::Sema;
use crate::conv::bases::{is_derived_from, same_class};

/// How a list query checks access (from an access context).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Checked {
    /// Null scope: everything is accessible.
    Unchecked,
    /// The designating class is unrelated: nothing is.
    Nothing,
    /// From the function, class or namespace `from`, the members named
    /// in class `naming`.
    From { from: DeclId, naming: DeclId },
}

/// Argument `i`, an `access_context` (scope in the low 64 bits, the
/// designating class in the high; [`kiln_oven::Shape::Pair`]).
pub(super) fn access_ctx_arg(
    args: &[u128],
    i: usize,
) -> Result<(ReflectionId, ReflectionId), MetaError> {
    let v = *args.get(i).ok_or_else(MetaError::invalid)?;
    let lo = ReflectionId::from_bits(u128::from(v as u64)).ok_or_else(MetaError::invalid)?;
    let hi = ReflectionId::from_bits(v >> 64).ok_or_else(MetaError::invalid)?;
    Ok((lo, hi))
}

impl Sema<'_, '_> {
    pub(super) fn meta_access(
        &mut self,
        a: AccessOp,
        args: &[u128],
        out: &mut MetaOut,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<(), MetaError> {
        let loc = ctx.loc;
        *out = match a {
            AccessOp::Current => {
                let s = self.current_scope(ctx);
                let r = self.scope_reflection(s, loc)?;
                MetaOut::Pair(u128::from(r.bits()), 0)
            }
            AccessOp::CurrentFunction => {
                let s = self.current_scope(ctx);
                if !self.kind_of(s).is_function() {
                    return fail(loc, "current scope does not represent a function");
                }
                MetaOut::Info(self.scope_reflection(s, loc)?)
            }
            AccessOp::CurrentClass => {
                let s = self.current_scope(ctx);
                let class = |k| {
                    matches!(
                        k,
                        DeclKind::CXXRecord
                            | DeclKind::ClassTemplateSpecialization
                            | DeclKind::ClassTemplatePartialSpecialization
                    )
                };
                let c = if class(self.kind_of(s)) {
                    Some(s)
                } else if self.kind_of(s).is_function() {
                    self.ast.decls.get(s).ctx().filter(|&c| class(self.kind_of(c)))
                } else {
                    None
                };
                let Some(c) = c else {
                    return fail(
                        loc,
                        "current scope does not represent a class nor a member function",
                    );
                };
                MetaOut::Info(self.scope_reflection(c, loc)?)
            }
            AccessOp::CurrentNamespace => {
                let mut s = Some(self.current_scope(ctx));
                for _ in 0..10_000 {
                    let Some(c) = s else { break };
                    if matches!(self.kind_of(c), DeclKind::Namespace | DeclKind::TranslationUnit) {
                        break;
                    }
                    s = self.ast.decls.get(c).ctx();
                }
                let s = s.unwrap_or(self.ast.tu);
                MetaOut::Info(self.scope_reflection(s, loc)?)
            }
            AccessOp::KilnVia => {
                let r = info_arg(args, 0)?;
                if r != ReflectionId::NULL && self.complete_class_of(r, ctx, oven)?.is_none() {
                    return fail(
                        loc,
                        "via argument other than null or complete class type reflection",
                    );
                }
                MetaOut::Info(r)
            }
            AccessOp::IsAccessible => {
                let (r, actx) = (info_arg(args, 0)?, access_ctx_arg(args, 1)?);
                MetaOut::Scalar(u128::from(self.is_accessible_refl(r, actx, loc)?))
            }
            AccessOp::HasInaccessibleNonstaticDataMembers
            | AccessOp::HasInaccessibleBases
            | AccessOp::HasInaccessibleSubobjects => {
                let (r, actx) = (info_arg(args, 0)?, access_ctx_arg(args, 1)?);
                let Some(def) = self.complete_class_of(r, ctx, oven)? else {
                    return fail(loc, "not a complete class type");
                };
                MetaOut::Scalar(u128::from(self.has_inaccessible(a, def, actx, ctx, oven)?))
            }
        };
        Ok(())
    }

    /// The function, class or namespace whose scope the evaluation point
    /// is immediately in ([meta.reflection.scope]/5, CURRENT-SCOPE): the
    /// function whose body holds the call, else the context the
    /// constant expression is in. A consteval block's body is not a
    /// function of its own: the evaluation point is in the scope the
    /// outermost enclosing consteval-block-declaration inhabits
    /// ([meta.reflection.scope]/3.5), so the walk goes on from the
    /// block's synthesized lambda to the context of its closure type.
    fn current_scope(&self, ctx: &MetaCtx) -> DeclId {
        let mut cur = Some(ctx.scope.unwrap_or_else(|| self.constraint_scope()));
        for _ in 0..10_000 {
            let Some(c) = cur else { break };
            let k = self.kind_of(c);
            if k.is_function()
                && let Some(closure) = self.consteval_block_closure(c)
            {
                cur = self.ast.decls.get(closure).ctx();
                continue;
            }
            // An inheriting constructor has no body: a call in its frame
            // is in a default member initializer it uses, whose point is
            // in the class scope ([meta.reflection.access.context]/1.1.2).
            if k == DeclKind::CXXConstructor && self.inherited_ctor_base(c).is_some() {
                cur = self.ast.decls.get(c).ctx();
                continue;
            }
            if k.is_function()
                || matches!(
                    k,
                    DeclKind::CXXRecord
                        | DeclKind::ClassTemplateSpecialization
                        | DeclKind::ClassTemplatePartialSpecialization
                        | DeclKind::Namespace
                        | DeclKind::TranslationUnit
                )
            {
                return c;
            }
            cur = self.ast.decls.get(c).ctx();
        }
        self.ast.tu
    }

    /// The evaluation point's scope outside any function: while the
    /// associated constraints of a declaration `D` (a trailing or
    /// template-head requires-clause) or a concept's constraint are being
    /// checked, the innermost scope enclosing `D`'s locus that is not a
    /// template parameter scope ([meta.reflection.access.context]/1.3:
    /// a class template member's is its class); else the context the
    /// constant expression is in.
    fn constraint_scope(&self) -> DeclId {
        use crate::templ::stack::FrameKind;
        let frame =
            self.inst.stack.frames.iter().rev().find(|f| !f.kind.is_constraint_substitution());
        match frame {
            Some(f) if matches!(f.kind, FrameKind::ConstraintsCheck | FrameKind::ConceptCheck) => {
                self.ast.decls.get(f.entity).ctx().unwrap_or_else(|| self.access_start())
            }
            _ => self.access_start(),
        }
    }

    /// The closure type of the consteval block whose synthesized lambda
    /// has call operator `f`, if it is one: the parser gives that lambda
    /// an introducer whose `[` and `]` are both the `consteval` keyword
    /// (`kiln_parse` `parse_consteval_block_call`), which no written
    /// lambda has (its brackets are two tokens), and a substituted lambda
    /// keeps its pattern's introducer.
    pub(crate) fn consteval_block_closure(&self, f: DeclId) -> Option<DeclId> {
        let id = self.ast.call_op_lambda(f)?;
        let data = self.ast.exprs.lambdas.get(id);
        (data.lbracket.is_valid() && data.lbracket == data.rbracket).then_some(data.closure)
    }

    /// The reflection of scope `s` (a function, class or namespace).
    fn scope_reflection(&mut self, s: DeclId, loc: Loc) -> Result<ReflectionId, MetaError> {
        let r = self.reflectee_of_decl(s).unwrap_or(Reflectee::Decl(self.entity_decl(s)));
        self.meta_intern(r, loc)
    }

    /// `is_accessible(r, ctx)` (P2996R13 [meta.reflection.access.queries]/3).
    /// Constant when the member's class (a base's derived class) is
    /// complete here ([meta.reflection.access.queries]/2).
    fn is_accessible_refl(
        &mut self,
        r: ReflectionId,
        actx: (ReflectionId, ReflectionId),
        loc: Loc,
    ) -> Result<bool, MetaError> {
        let (scope, designating) = actx;
        // The member (or base-specifier) and its class.
        let (decl, parent, base) = match self.ast.reflections.get(r) {
            Reflectee::BaseSpec(d, i) => {
                let def = self.ast.definition(d).unwrap_or(d);
                let Some(spec) = self.base_spec_index(def, i) else { return Ok(true) };
                let ty = self.ast.decls.base_spec(spec).ty;
                let Some(b) = self.ast.types.as_record(ty) else { return Ok(true) };
                (self.ast.definition(b).unwrap_or(b), def, Some(i))
            }
            other => {
                let Some(d) = self.reflected_member_decl(other) else { return Ok(true) };
                let Some(p) = self.declaring_class(d) else { return Ok(true) };
                (d, self.ast.definition(p).unwrap_or(p), None)
            }
        };
        if !self.is_complete(self.tag_type(parent)) {
            return fail(loc, "incomplete parent class");
        }
        let nc = self.reflected_class_def(designating).unwrap_or(parent);
        // A member of the designating class (inherited included); a base
        // of it or of one of its bases.
        let member_of = |s: &Self, c: DeclId| same_class(s, nc, c) || is_derived_from(s, nc, c);
        let mut decl = decl;
        if !member_of(self, parent) {
            // A variant (or anonymous struct) member of the designating
            // class ("possibly indirect or variant member"): a member of
            // the enclosing named class ([class.union.anon]/1), with the
            // access of the anonymous object that class declares.
            let ind = match base {
                None => self.anon_member_as_indirect(decl).ok().filter(|&m| m != decl),
                Some(_) => None,
            };
            let obj = ind.and_then(|m| {
                self.ast.lists.iter::<DeclId>(self.ast.indirect_field_chain(m)).next()
            });
            let outer = obj.and_then(|m| self.declaring_class(m));
            match (obj, outer) {
                (Some(m), Some(c)) if member_of(self, self.ast.definition(c).unwrap_or(c)) => {
                    decl = m;
                }
                _ => return Ok(false),
            }
        }
        let Some(from) = self.scope_decl(scope) else { return Ok(true) };
        let mut sc = std::mem::take(&mut self.meta_members.scope);
        self.set_access_scope(&mut sc, from);
        let ok = match base {
            Some(i) => match self.base_spec_index(parent, i) {
                Some(spec) => self.base_accessible_in(&sc, nc, parent, spec, decl),
                None => true,
            },
            None => self.member_accessible_in(&sc, decl, nc),
        };
        self.meta_members.scope = sc;
        Ok(ok)
    }

    /// The declaration of the class member reflectee `r` designates, if
    /// any (a member declaration, or a member type, alias or template).
    fn reflected_member_decl(&self, r: Reflectee) -> Option<DeclId> {
        use kiln_ast::types::TypeView;
        match r {
            Reflectee::Decl(d) => Some(d),
            Reflectee::Type(t) => match self.ast.types.view(t.ty()) {
                TypeView::Typedef { decl, .. } => Some(decl),
                _ => self.ast.types.as_record(t).or_else(|| self.ast.types.as_enum(t)),
            },
            Reflectee::Template(n) => self.ast.types.template_decl(n),
            _ => None,
        }
    }

    /// `has_inaccessible_nonstatic_data_members` / `_bases` /
    /// `_subobjects` of complete class `def`: whether an element of the
    /// unchecked list is inaccessible in `actx`.
    fn has_inaccessible(
        &mut self,
        a: AccessOp,
        def: DeclId,
        actx: (ReflectionId, ReflectionId),
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<bool, MetaError> {
        // Build the lists (the unchecked query), then test each entry.
        self.class_lists(def, MemberOp::MembersOf, &Checked::Unchecked, oven, ctx.loc)?;
        let chk = self.checked(actx, def, ctx);
        let mut sc = std::mem::take(&mut self.meta_members.scope);
        if let Checked::From { from, .. } = chk {
            self.set_access_scope(&mut sc, from);
        }
        let st = &self.meta_members.cache;
        let list = |k| st.get(def, k).map_or(&[][..], |l: &kiln_oven::CachedList| &l.items[..]);
        let members = list(kiln_oven::MemberList::Members);
        let bases = list(kiln_oven::MemberList::Bases);
        let bad_member = || {
            members
                .iter()
                .any(|e| e.kind & mk::NSDM != 0 && !self.entry_accessible(e, def, false, &chk, &sc))
        };
        let bad_base = || bases.iter().any(|e| !self.entry_accessible(e, def, true, &chk, &sc));
        let r = match a {
            AccessOp::HasInaccessibleNonstaticDataMembers => bad_member(),
            AccessOp::HasInaccessibleBases => bad_base(),
            _ => bad_base() || bad_member(),
        };
        self.meta_members.scope = sc;
        Ok(r)
    }
}
