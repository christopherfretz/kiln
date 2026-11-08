//! The annotation queries ([meta.reflection.annotation];
//! spec/annotations.md §4, spec/reflection-members.md §5; WP-478):
//! `annotations_of`, `annotations_of_with_type` and `is_annotation`, read
//! from the annotation side table (`Ast::annots`).

use kiln_ast::decl::DeclKind;
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::TypeView;
use kiln_ast::{AnnId, DeclId, QualType, ReflectionId};
use kiln_oven::{AnnotOp, MetaCtx, MetaError, MetaOut, Oven, info_arg};
use kiln_source::Loc;

use super::fail;
use crate::Sema;

/// What `annotations_of` reads for one reflection: the declarations of
/// an entity, or the `i`th parameter of each declaration of a function.
#[derive(Clone, Copy)]
enum Source {
    Entity(DeclId),
    Param(DeclId, u32),
    /// The `i`th direct base-specifier of a class (first declaration).
    Base(DeclId, u32),
    None,
}

impl Sema<'_, '_> {
    pub(super) fn meta_annot(
        &mut self,
        a: AnnotOp,
        args: &[u128],
        out: &mut MetaOut,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<(), MetaError> {
        let loc = ctx.loc;
        let r = info_arg(args, 0)?;
        *out = match a {
            AnnotOp::IsAnnotation => MetaOut::Scalar(u128::from(matches!(
                self.ast.reflections.get(r),
                Reflectee::Annotation(_)
            ))),
            AnnotOp::AnnotationsOf => MetaOut::Infos(self.annotations_of(r, None, ctx, oven)?),
            AnnotOp::AnnotationsOfWithType => {
                let t = info_arg(args, 1)?;
                let ty = match self.ast.reflections.get(t) {
                    Reflectee::Type(q) => {
                        if self.undefined_spec(q).is_some() && self.meta_may_instantiate(ctx) {
                            self.lend_oven(oven, |s| s.require_complete(q, loc));
                        }
                        Some(q).filter(|&q| self.is_complete(q))
                    }
                    _ => None,
                };
                let Some(ty) = ty else {
                    return fail(
                        loc,
                        "reflection does not represent a complete type or type alias",
                    );
                };
                // remove_const of both sides (P3394R4): an annotation's
                // type is cv-unqualified, so a volatile type matches none.
                let c = self.ast.types.canonical(ty);
                let want = QualType::new(c.ty(), c.quals_bits() & !1);
                MetaOut::Infos(self.annotations_of(r, Some(want), ctx, oven)?)
            }
        };
        Ok(())
    }

    /// What `annotations_of(r)` reads (`Err`: an unsuitable reflection).
    fn annotation_source(&self, r: ReflectionId, loc: Loc) -> Result<Source, MetaError> {
        Ok(match self.ast.reflections.get(r) {
            Reflectee::Decl(d) => match self.kind_of(d) {
                DeclKind::Var
                | DeclKind::VarTemplateSpecialization
                | DeclKind::Field
                | DeclKind::EnumConstant
                | DeclKind::ParmVar => Source::Entity(d),
                k if k.is_function() => Source::Entity(d),
                _ => return Self::annotation_unsuitable(loc),
            },
            Reflectee::FnParam(f, i) => Source::Param(f, i),
            Reflectee::Namespace(d) => Source::Entity(d),
            // A namespace alias has its own annotations, and a
            // namespace-alias-definition admits none: always empty, never
            // the aliased namespace's (GCC 16.1 agrees; WP-573).
            Reflectee::NamespaceAlias(_) => Source::None,
            Reflectee::BaseSpec(c, i) => Source::Base(c, i),
            Reflectee::Type(t) => match self.ast.types.view(t.ty()) {
                // An alias has its own annotations.
                TypeView::Typedef { decl, .. } => Source::Entity(decl),
                _ => match self.ast.types.as_record(t).or_else(|| self.ast.types.as_enum(t)) {
                    Some(d) => Source::Entity(d),
                    None => Source::None,
                },
            },
            _ => return Self::annotation_unsuitable(loc),
        })
    }

    fn annotation_unsuitable<T>(loc: Loc) -> Result<T, MetaError> {
        fail(
            loc,
            "reflection does not represent a type, type alias, variable, function, function \
             parameter, namespace, enumerator, direct base class relationship, or non-static \
             data member",
        )
    }

    /// `annotations_of(r)` (with `want`: `annotations_of_with_type`,
    /// keeping those whose type is `want`, the given type without const) into the Oven's
    /// result buffer: the annotations of every declaration of the
    /// entity, first to latest, each in source order.
    fn annotations_of(
        &mut self,
        r: ReflectionId,
        want: Option<QualType>,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<u32, MetaError> {
        let loc = ctx.loc;
        // A class template specialization's annotations are its
        // instantiation's ([dcl.attr.annotation]/3).
        if let Reflectee::Type(t) = self.ast.reflections.get(r)
            && self.undefined_spec(t).is_some()
        {
            if !self.meta_may_instantiate(ctx) {
                return Err(MetaError::silent());
            }
            self.lend_oven(oven, |s| s.require_complete(t, loc));
        }
        let src = self.annotation_source(r, loc)?;
        let f = match src {
            Source::Param(f, _) => Some(f),
            Source::Entity(d) if self.kind_of(d).is_function() => Some(d),
            _ => None,
        };
        if let Some(f) = f {
            let first = self.ast.first(f);
            if self.uninstantiated_annots.contains_key(&first) {
                if !self.meta_may_instantiate(ctx) {
                    return Err(MetaError::silent());
                }
                self.lend_oven(oven, |s| s.require_annotations(f, loc));
            }
        }
        let mut decls: Vec<DeclId> = match src {
            Source::Entity(d) | Source::Param(d, _) => self.ast.redecls_of(d).collect(),
            // A base-specifier is written once, in the class definition.
            Source::Base(c, _) => vec![c],
            Source::None => Vec::new(),
        };
        decls.reverse();
        let mut v = std::mem::take(oven.meta_infos());
        v.clear();
        for d in decls {
            let on = match src {
                Source::Param(_, i) => self
                    .ast
                    .decls
                    .function(d)
                    .and_then(|x| self.ast.lists.iter::<DeclId>(x.params).nth(i as usize)),
                _ => Some(d),
            };
            let Some(on) = on else { continue };
            let list = match src {
                Source::Base(c, i) => self.ast.annots.of_base(c, i),
                _ => self.ast.annots.of_decl(on),
            };
            let n = self.ast.lists.get(list).len();
            for k in 0..n {
                let Some(a) = self.ast.lists.iter::<AnnId>(list).nth(k) else { continue };
                if let Some(w) = want {
                    let t = self.ast.annots.get(a).written_type;
                    if self.ast.types.canonical(t).without_quals() != w {
                        continue;
                    }
                }
                v.push(self.meta_intern(Reflectee::Annotation(a), loc)?);
            }
        }
        let n = u32::try_from(v.len()).unwrap_or(u32::MAX);
        *oven.meta_infos() = v;
        Ok(n)
    }
}
