//! The kiln extensions (spec/reflection.md §3.4): the spike's `name_of`,
//! kept until WP-475's `identifier_of` replaces it (WP-478 retired the
//! spike's member ops for `members_of`), and `__kiln_define_member`,
//! which stays an extension (D-ext).

use kiln_ast::reflect::Reflectee;
use kiln_ast::{ConstValueId, ReflectionId};
use kiln_diag::DiagId;
use kiln_oven::{Dms, ExtOp, MetaCtx, MetaError, MetaOut, Note, Oven, info_arg};
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
            ExtOp::DefineMember => {
                let (cls, ty) = (info_arg(args, 0)?, info_arg(args, 1)?);
                let p = *args.get(2).ok_or_else(MetaError::invalid)?;
                let name = oven.meta_c_bytes(p, ctx)?;
                MetaOut::Info(self.define_member(cls, ty, &name, ctx, oven)?)
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
        name: &[u8],
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<ReflectionId, MetaError> {
        let loc = ctx.loc;
        // A fold or a trial must not mutate (the dispatcher already
        // fails a `Mutating` op there): it fails silently and the
        // manifestly constant-evaluated evaluation (if any) defines. The
        // extension may inject from any manifestly constant-evaluated
        // context in the member-specification (a `static_assert`), not
        // only where `ctx.plainly` holds (spec/consteval-blocks.md §3).
        if ctx.trial || self.inst.eval_loc.is_none() {
            return Err(MetaError::silent());
        }
        // `-Wkiln-extension` (default on; spec/reflection.md §6.3).
        let w = self.diag(DiagId::ext_kiln_define_member, loc);
        self.emit(w);
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
        let Some(name) = std::str::from_utf8(name).ok().filter(|s| is_identifier(s)) else {
            let shown = kiln_diag::escape_diag_text(name);
            return fail(loc, format!("'{shown}' is not an identifier"));
        };
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
            let mut notes = vec![
                Note::new(loc, format!("duplicate member '{name}'")),
                Note::new(ploc, "previous declaration is here"),
            ];
            if let Some(&at) = self.ast.decls.cold.injection_point.get(prev) {
                notes.push(Note::new(at, "member injected here"));
            }
            notes.push(in_eval);
            return Err(notes.into());
        }
        // `define_aggregate`'s injection path (spec/reflection.md §6.2):
        // the member is placed where the evaluation is.
        let d =
            Dms { ty: t, name: Some(sym), alignment: None, bit_width: None, nua: false, annots: 0 };
        let point = self.inst.eval_loc.unwrap_or(loc);
        let f = self.inject_field(r, &d, &[], loc, point);
        Ok(self.intern_reflection(Reflectee::Decl(f), loc)?)
    }
}
