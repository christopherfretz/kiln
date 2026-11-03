//! The kiln extensions (spec/reflection.md §3.4): the spike's ops, kept
//! until their IS replacements land (`name_of` → `identifier_of`,
//! WP-475; `__kiln_members_count`/`__kiln_member_at` → `members_of`,
//! WP-478; `__kiln_define_member` stays an extension, D-ext).

use kiln_ast::decl::{Access, DeclFlags, DeclKind};
use kiln_ast::names::DeclName;
use kiln_ast::reflect::Reflectee;
use kiln_ast::{ConstValueId, DeclHead, DeclId, ReflectionId};
use kiln_oven::{ExtOp, MetaCtx, MetaError, MetaOut, Note, Oven, info_arg};
use kiln_source::Loc;

use super::{fail, is_identifier};
use crate::Sema;
use crate::lookup::LookupKind;

impl Sema<'_, '_> {
    pub(super) fn meta_ext(
        &mut self,
        x: ExtOp,
        args: &[u128],
        out: &mut MetaOut,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<(), MetaError> {
        let loc = ctx.loc;
        *out = match x {
            ExtOp::NameOf => MetaOut::Frozen(self.reflect_name(info_arg(args, 0)?, loc)?),
            ExtOp::MembersCount => {
                let m = self.reflect_members(info_arg(args, 0)?, loc, oven)?;
                MetaOut::Scalar(m.len() as u128)
            }
            ExtOp::MemberAt => {
                let i = *args.get(1).ok_or_else(MetaError::invalid)?;
                let m = self.reflect_members(info_arg(args, 0)?, loc, oven)?;
                match usize::try_from(i).ok().and_then(|i| m.get(i)) {
                    Some(&r) => MetaOut::Info(r),
                    None => {
                        return fail(
                            loc,
                            format!(
                                "member index {i} is out of range for a class with {} members",
                                m.len()
                            ),
                        );
                    }
                }
            }
            ExtOp::DefineMember => {
                let (cls, ty) = (info_arg(args, 0)?, info_arg(args, 1)?);
                let p = *args.get(2).ok_or_else(MetaError::invalid)?;
                let name = oven.meta_c_string(p, ctx)?;
                MetaOut::Info(self.define_member(cls, ty, &name, loc, oven)?)
            }
        };
        Ok(())
    }

    /// `name_of(r)`: the identifier of a declaration, or a type's name.
    /// The spike returns a `const char *` to a string literal object
    /// (one per content, shared with `std::source_location`'s).
    fn reflect_name(&mut self, r: ReflectionId, loc: Loc) -> Result<ConstValueId, MetaError> {
        let name = match self.ast.reflections.get(r) {
            Reflectee::Decl(d) | Reflectee::Namespace(d) | Reflectee::NamespaceAlias(d) => {
                self.decl_name(d)
            }
            Reflectee::Type(t) => match self.ast.types.as_record(t) {
                Some(d) => self.decl_name(d),
                None => self.ty_str(t),
            },
            Reflectee::Template(n) => {
                self.ast.types.template_decl(n).map(|d| self.decl_name(d)).unwrap_or_default()
            }
            _ => String::new(),
        };
        if name.is_empty() {
            return fail(loc, format!("'{}' has no name", self.reflection_str(r)));
        }
        match self.srcloc_string(&name) {
            Some(cv) => Ok(cv),
            None => fail(loc, "kiln: name too large for a constant"),
        }
    }

    /// The class type reflected by `r`, completed (instantiated at `loc`
    /// if it is an implicitly instantiable specialization; `oven` lent).
    fn reflected_complete_class(
        &mut self,
        r: ReflectionId,
        loc: Loc,
        oven: &mut Oven,
    ) -> Result<DeclId, MetaError> {
        let not_class = |s: &Self| {
            MetaError::note(loc, format!("'{}' does not reflect a class type", s.reflection_str(r)))
        };
        let Reflectee::Type(t) = self.ast.reflections.get(r) else { return Err(not_class(self)) };
        let Some(d) = self.ast.types.as_record(t) else { return Err(not_class(self)) };
        if self.undefined_spec(t).is_some() {
            // Not manifestly constant-evaluated: no instantiation from a
            // trial (WP-473 moves this to the dispatcher's trial rule).
            if self.inst.eval_loc.is_none() {
                return Err(MetaError::silent());
            }
            self.lend_oven(oven, |s| s.require_complete(t, loc));
        }
        match self.ast.definition(d) {
            Some(def) if self.is_complete(t) => Ok(def),
            _ => fail(loc, format!("'{}' reflects an incomplete class", self.reflection_str(r))),
        }
    }

    /// The members of the class `r` reflects, in declaration order
    /// (the spike's `members_of`): every named member declared in the
    /// class (an implicitly declared special member is excluded: kiln
    /// declares those lazily, so whether one exists depends on what was
    /// used before; the IS includes them, WP-478).
    fn reflect_members(
        &mut self,
        r: ReflectionId,
        loc: Loc,
        oven: &mut Oven,
    ) -> Result<Vec<ReflectionId>, MetaError> {
        let def = self.reflected_complete_class(r, loc, oven)?;
        let members: Vec<DeclId> = self
            .ast
            .members(def)
            .filter(|&m| {
                let n = self.ast.decls.get(m);
                !n.has(DeclFlags::IMPLICIT)
                    && !self.decl_name(m).is_empty()
                    && !matches!(
                        n.kind(),
                        DeclKind::AccessSpec | DeclKind::StaticAssert | DeclKind::Friend
                    )
            })
            .collect();
        // Each member keyed as `^^` keys it (a nested type or alias is a
        // type, a member template a template; spec/reflection-repr.md §1),
        // one reflection per entity (`Sema::entity_decl`).
        members
            .into_iter()
            .map(|m| {
                let r = self.reflectee_of_decl(m).unwrap_or(Reflectee::Decl(self.entity_decl(m)));
                self.intern_reflection(r, loc).map_err(MetaError::from)
            })
            .collect()
    }

    /// `__kiln_define_member(cls, ty, name)` (spec/reflection.md §3.4):
    /// appends the public non-static data member `name` of type `ty` to
    /// `cls` while the Oven evaluates. Permitted only in a manifestly
    /// constant-evaluated context while `cls` is the innermost class
    /// being defined (its member-specification is being processed: a
    /// `static_assert` or another constant expression among its member
    /// declarations, including an instantiation of it); the member is
    /// placed where the evaluation happens in the member-specification.
    fn define_member(
        &mut self,
        cls: ReflectionId,
        ty: ReflectionId,
        name: &str,
        loc: Loc,
        oven: &mut Oven,
    ) -> Result<ReflectionId, MetaError> {
        // A speculative fold must not mutate: it fails silently and the
        // manifestly constant-evaluated evaluation (if any) defines.
        if self.inst.eval_loc.is_none() {
            return Err(MetaError::silent());
        }
        let Some(r) = (match self.ast.reflections.get(cls) {
            Reflectee::Type(t) => self.ast.types.as_record(t),
            _ => None,
        }) else {
            return fail(
                loc,
                format!("'{}' does not reflect a class type", self.reflection_str(cls)),
            );
        };
        // `^^S` names the first declaration of `S`; the class being
        // defined is its definition: compare entities, not decls.
        let building = self.classes.last().map(|c| c.record);
        let Some(r) = building.filter(|&b| self.same_entity(b, r)) else {
            let cname = self.decl_name(r);
            let mut notes = vec![Note::new(
                loc,
                format!(
                    "'__kiln_define_member' can only add a member to the class whose \
                     member-specification is being evaluated ('{cname}' is not)"
                ),
            )];
            if let Some(b) = building {
                let bloc = self.ast.decls.get(b).loc();
                notes.push(Note::new(
                    bloc,
                    format!("class being defined is '{}'", self.decl_name(b)),
                ));
            }
            return Err(notes.into());
        };
        let Reflectee::Type(t) = self.ast.reflections.get(ty) else {
            return fail(loc, format!("'{}' does not reflect a type", self.reflection_str(ty)));
        };
        if self.undefined_spec(t).is_some() {
            self.lend_oven(oven, |s| s.require_complete(t, loc));
        }
        if self.ast.types.is_function(t)
            || self.ast.types.is_reference(t)
            || !self.is_complete(t)
            || self.ast.types.as_record(t).is_some_and(|d| self.same_entity(d, r))
        {
            return fail(
                loc,
                format!(
                    "'{}' is not a complete object type for a data member",
                    self.reflection_str(ty)
                ),
            );
        }
        if !is_identifier(name) {
            return fail(loc, format!("'{name}' is not an identifier"));
        }
        let sym = self.ast.symbols().intern(name);
        let class_loc = self.ast.decls.get(r).loc();
        let in_eval = Note::new(
            class_loc,
            format!(
                "in evaluation of '__kiln_define_member' for class '{}' being defined here",
                self.decl_name(r)
            ),
        );
        if let Some(&prev) = self.lookup_in(r, sym, LookupKind::Ordinary).decls.first() {
            let ploc = self.ast.decls.get(prev).loc();
            return Err(vec![
                Note::new(loc, format!("duplicate member '{name}'")),
                Note::new(ploc, "previous declaration is here"),
                in_eval,
            ]
            .into());
        }
        let mut h = DeclHead::new(r, DeclName::ident(sym), loc);
        h.flags = DeclFlags::INJECTED;
        let f = self.builder().field(h, t, None, 0);
        self.ast.decls.set_access(f, Access::Public);
        Ok(self.intern_reflection(Reflectee::Decl(f), loc)?)
    }
}
