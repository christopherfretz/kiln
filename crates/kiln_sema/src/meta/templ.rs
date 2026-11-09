//! The template family ([meta.reflection.substitute]; spec/reflection.md
//! §3.3 Template rows): `template_of`, `template_arguments_of`,
//! `substitute` and `can_substitute`.

use kiln_ast::consts::{PtrBase, Scalar};
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::{ArgView, Quals, TemplateArg, TypeView};
use kiln_ast::{ArgListId, ConstValueId, DeclId, QualType, ReflectionId, TemplateNameId};
use kiln_diag::{Diag, DiagId};
use kiln_oven::{MetaCtx, MetaError, MetaOut, Oven, TemplOp, info_arg};
use kiln_source::Loc;
use kiln_source::fx::FxHashMap;

use super::fail;
use crate::Sema;
use crate::templ::actions::TemplateIdRef;
use crate::templ::stack::FrameKind;

/// The `substitute` memo (spec/reflection.md §3.3): the reflection of
/// each successful substitution, keyed by the template name and the
/// argument reflections' Oven values.
#[derive(Debug, Default)]
pub(crate) struct SubstMemo {
    map: FxHashMap<Box<[u64]>, ReflectionId>,
    /// The key scratch (its capacity is reused).
    key: Vec<u64>,
    /// Calls answered from the memo.
    pub(crate) hits: u64,
}

/// How deep a template argument pack nests at most (a bound on a
/// malformed list).
const MAX_PACK_DEPTH: usize = 64;

impl Sema<'_, '_> {
    pub(super) fn meta_templ(
        &mut self,
        t: TemplOp,
        args: &[u128],
        out: &mut MetaOut,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<(), MetaError> {
        let loc = ctx.loc;
        *out = match t {
            TemplOp::TemplateOf => {
                let r = info_arg(args, 0)?;
                let (n, _) = self.specialization_of(r, loc)?;
                let n = self.ast.types.canonical_template_name(n);
                MetaOut::Info(self.intern_reflection(Reflectee::Template(n), loc)?)
            }
            TemplOp::TemplateArgumentsOf => {
                let r = info_arg(args, 0)?;
                let (_, l) = self.specialization_of(r, loc)?;
                let mut v = std::mem::take(oven.meta_infos());
                v.clear();
                let res = self.arg_reflections(l, &mut v, loc, 0);
                let n = u32::try_from(v.len()).map_err(|_| MetaError::invalid());
                *oven.meta_infos() = v;
                res?;
                MetaOut::Infos(n?)
            }
            TemplOp::CanSubstitute | TemplOp::Substitute => {
                return self.meta_substitute(t, args, out, ctx, oven);
            }
        };
        Ok(())
    }

    /// The template and the (canonical, converted) arguments of
    /// specialization reflection `r`: a class, alias, function or variable
    /// template specialization (an alias of a class template
    /// specialization is not one, as `has_template_arguments`). Never
    /// instantiates: the specialization record exists once it is named.
    fn specialization_of(
        &mut self,
        r: ReflectionId,
        loc: Loc,
    ) -> Result<(TemplateNameId, ArgListId), MetaError> {
        let found = match self.ast.reflections.get(r) {
            Reflectee::Type(t) if t.quals_bits() == 0 => match self.ast.types.view(t.ty()) {
                TypeView::TemplateSpecialization { name, args, alias: true, .. } => {
                    Some((name, args))
                }
                TypeView::Typedef { .. } => None,
                _ => self.ast.types.as_record(t).and_then(|d| self.spec_template_args(d)),
            },
            Reflectee::Type(t) => {
                self.ast.types.as_record(t).and_then(|d| self.spec_template_args(d))
            }
            Reflectee::Decl(d)
                if matches!(
                    self.kind_of(d),
                    DeclKind::Function
                        | DeclKind::CXXMethod
                        | DeclKind::CXXConstructor
                        | DeclKind::CXXConversion
                        | DeclKind::Var
                        | DeclKind::VarTemplateSpecialization
                ) =>
            {
                self.spec_template_args(d)
            }
            _ => None,
        };
        match found {
            Some((n, l)) => Ok((n, self.ast.types.canonical_args(l))),
            None => {
                fail(loc, format!("'{}' is not a template specialization", self.reflection_str(r)))
            }
        }
    }

    /// The template name and converted arguments of specialization `d`.
    fn spec_template_args(&mut self, d: DeclId) -> Option<(TemplateNameId, ArgListId)> {
        let (_, s) = self.ast.spec_info(d)?;
        let (tmpl, args) = (s.template, s.args);
        match self.reflectee_of_decl(tmpl)? {
            Reflectee::Template(n) => Some((n, args)),
            _ => None,
        }
    }

    /// Appends the reflections of argument list `l` to `out`, a pack's
    /// elements in place of the pack ([meta.reflection.substitute]:
    /// `template_arguments_of`).
    fn arg_reflections(
        &mut self,
        l: ArgListId,
        out: &mut Vec<ReflectionId>,
        loc: Loc,
        depth: usize,
    ) -> Result<(), MetaError> {
        if depth > MAX_PACK_DEPTH {
            return Err(MetaError::invalid());
        }
        let n = self.ast.types.args(l).len();
        for i in 0..n {
            let a = self.ast.types.args(l)[i];
            let x = match a.view() {
                ArgView::Type(t) => Reflectee::Type(self.ast.reflection_type(t)),
                ArgView::Value { ty, value } => self.value_arg_reflectee(ty, value, loc)?,
                ArgView::Template(n) => {
                    Reflectee::Template(self.ast.types.canonical_template_name(n))
                }
                ArgView::Pack(p) => {
                    self.arg_reflections(p, out, loc, depth + 1)?;
                    continue;
                }
                // A non-dependent specialization has no other argument.
                _ => return fail(loc, "a dependent template argument has no reflection"),
            };
            out.push(self.intern_reflection(x, loc)?);
        }
        Ok(())
    }

    /// The reflection of constant template argument `value` of parameter
    /// type `ty` ([meta.reflection.substitute]/`template_arguments_of`,
    /// keyed as `reflect_constant` and `reflect_object` key theirs): a
    /// reference's referent object (or function), a class type's
    /// template parameter object, else the value.
    fn value_arg_reflectee(
        &mut self,
        ty: QualType,
        value: ConstValueId,
        loc: Loc,
    ) -> Result<Reflectee, MetaError> {
        let types = &self.ast.types;
        if types.is_reference(ty)
            && let Some(referent) = types.pointee(ty)
        {
            return match self.ast.consts.scalar(value) {
                Some(Scalar::Pointer { base: PtrBase::Function(f), .. }) => {
                    Ok(Reflectee::Decl(self.entity_decl(f)))
                }
                Some(Scalar::Pointer { base, offset, .. }) => {
                    Ok(Reflectee::Object(self.object_pointer(referent, base, offset, loc)?))
                }
                _ => Err(MetaError::invalid()),
            };
        }
        if types.as_record(ty).is_some() {
            let ct = self.ast.types.qualify(ty, Quals::CONST);
            let obj = self.builder().template_param_object(ct, value);
            return Ok(Reflectee::Object(self.object_pointer(ty, PtrBase::Decl(obj), 0, loc)?));
        }
        Ok(Reflectee::Value(value))
    }

    /// The interned pointer of an object reflection: to `base` + `offset`,
    /// of type pointer to `obj` cv-unqualified (spec/reflection-repr.md:
    /// one pointer, so one reflection, per object).
    fn object_pointer(
        &mut self,
        obj: QualType,
        base: PtrBase,
        offset: u64,
        loc: Loc,
    ) -> Result<ConstValueId, MetaError> {
        let obj = self.ast.types.canonical(obj).without_quals();
        let ty = self.ast.types.pointer_to(obj);
        self.ast
            .consts
            .push_scalar(Scalar::Pointer { ty, base, offset, designated: None, past: false })
            .map_err(|_| MetaError::note(loc, "too many constant values"))
    }

    /// `substitute(templ, args)` / `can_substitute(templ, args)`
    /// ([meta.reflection.substitute]/4-8; spec/reflection.md §3.3): forms
    /// the template-id `templ<args...>` through the template-id sema in a
    /// SFINAE frame, never instantiating a definition, as a transactional
    /// Oven trial that a failure rolls back (§4.3). A success is memoized
    /// on the (template, arguments) key; a failure is not (it may depend
    /// on the point of evaluation).
    fn meta_substitute(
        &mut self,
        t: TemplOp,
        args: &[u128],
        out: &mut MetaOut,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<(), MetaError> {
        let loc = ctx.loc;
        let probe = t == TemplOp::CanSubstitute;
        let r = info_arg(args, 0)?;
        let (n, td) = match self.ast.reflections.get(r) {
            Reflectee::Template(n) => match self.ast.types.template_decl(n) {
                Some(d) => (n, d),
                None => {
                    return fail(loc, format!("'{}' is not a template", self.reflection_str(r)));
                }
            },
            _ => return fail(loc, format!("'{}' is not a template", self.reflection_str(r))),
        };
        let range = *args.get(1).ok_or_else(MetaError::invalid)?;
        let len = oven.meta_range_len(range);
        let mut key = std::mem::take(&mut self.meta_subst.key);
        key.clear();
        key.push(u64::from(n.index()));
        let mut read = Ok(());
        for i in 0..len {
            match oven.meta_range_at(range, i) {
                Ok(a) => key.push(a.bits()),
                Err(e) => {
                    read = Err(e);
                    break;
                }
            }
        }
        let hit = self.meta_subst.map.get(key.as_slice()).copied();
        let res = match (read, hit) {
            (Err(e), _) => Err(e),
            (Ok(()), Some(x)) => {
                self.meta_subst.hits += 1;
                Ok(Ok(x))
            }
            (Ok(()), None) => self.substitute_uncached(n, td, &key[1..], loc, oven),
        };
        let res = match res {
            Ok(r) => r,
            Err(e) => {
                self.meta_subst.key = key;
                return Err(e);
            }
        };
        if let Ok(x) = res
            && hit.is_none()
        {
            self.meta_subst.map.insert(key.as_slice().into(), x);
        }
        self.meta_subst.key = key;
        *out = match (res, probe) {
            (Ok(_), true) => MetaOut::Scalar(1),
            (Err(_), true) => MetaOut::Scalar(0),
            (Ok(x), false) => MetaOut::Info(x),
            (Err(why), false) => {
                return fail(
                    loc,
                    format!(
                        "'{}' cannot be substituted with these template arguments: {why}",
                        self.reflection_str(r)
                    ),
                );
            }
        };
        Ok(())
    }

    /// The substitution of `n` (template `td`) with the reflections
    /// `bits`, not memoized: the result, or the reason it fails. An
    /// argument that cannot be a template argument at all is an error
    /// (of `can_substitute` too: its *Constant When*).
    fn substitute_uncached(
        &mut self,
        n: TemplateNameId,
        td: DeclId,
        bits: &[u64],
        loc: Loc,
        oven: &mut Oven,
    ) -> Result<Result<ReflectionId, String>, MetaError> {
        let mut targs = Vec::with_capacity(bits.len());
        for &b in bits {
            let a = ReflectionId::from_bits(u128::from(b)).ok_or_else(MetaError::invalid)?;
            targs.push(self.reflection_template_arg(a, loc)?);
        }
        oven.trial_begin();
        self.inst.report.begin_trial();
        let res = self.lend_oven(oven, |s| {
            s.in_sfinae(FrameKind::MetaSubstitution, td, None, loc, |s| {
                let saved = std::mem::take(&mut s.templ.arg_locs);
                let r = s.act_template_id(n, &targs, loc, loc);
                s.templ.arg_locs = saved;
                match r {
                    Ok(id) => s.template_id_reflectee(id),
                    Err(()) => Err(None),
                }
            })
        });
        let res = match res {
            None => Err("the template instantiation depth limit was reached".to_owned()),
            Some((Ok(x), Ok(()))) => Ok(x),
            Some((Err(Some(why)), _)) => Err(why.to_owned()),
            Some((_, Err(d))) => Err(substitution_reason(d.as_ref())),
            Some((Err(None), Ok(()))) => Err("substitution failure".to_owned()),
        };
        oven.trial_end(res.is_ok());
        self.inst.report.end_trial(res.is_ok());
        match res {
            Ok(x) => Ok(Ok(self.intern_reflection(x, loc)?)),
            Err(why) => Ok(Err(why)),
        }
    }

    /// The template argument reflection `r` stands for
    /// ([meta.reflection.substitute]/2): a type, a template, a value's
    /// literal, an lvalue naming an object or variable, or a function.
    fn reflection_template_arg(
        &mut self,
        r: ReflectionId,
        loc: Loc,
    ) -> Result<TemplateArg, MetaError> {
        let arg = match self.ast.reflections.get(r) {
            Reflectee::Type(t) => Some(TemplateArg::ty(t)),
            Reflectee::Template(n) => Some(TemplateArg::template(n)),
            Reflectee::Value(v) => {
                let ty = self.ast.consts.ty(v);
                self.nttp_arg_expr(ty, v, loc).map(|e| TemplateArg::expr(e, None))
            }
            // The lvalue naming the object, at any offset of a variable,
            // a template parameter object or an object no declaration
            // names, as its splice does ([meta.reflection.substitute]/
            // [temp.arg.nontype]; `splice_object.rs`).
            Reflectee::Object(p) => {
                self.splice_object_expr(p, loc).map(|e| TemplateArg::expr(e, None))
            }
            Reflectee::Decl(d)
                if matches!(
                    self.kind_of(d),
                    DeclKind::Var
                        | DeclKind::VarTemplateSpecialization
                        | DeclKind::Function
                        | DeclKind::CXXMethod
                        | DeclKind::EnumConstant
                ) =>
            {
                Some(TemplateArg::expr(self.build_decl_ref(d, loc), None))
            }
            _ => None,
        };
        match arg {
            Some(a) => Ok(a),
            None => {
                fail(loc, format!("'{}' cannot be a template argument", self.reflection_str(r)))
            }
        }
    }

    /// The reflectee of the entity template-id `id` names: the type, the
    /// variable or function template specialization, or the value of a
    /// concept-id. `Err(Some(reason))` when it names none.
    fn template_id_reflectee(
        &mut self,
        id: TemplateIdRef,
    ) -> Result<Reflectee, Option<&'static str>> {
        match id {
            TemplateIdRef::Type(t) => {
                if self.is_error_type(t) {
                    return Err(None);
                }
                Ok(Reflectee::Type(self.ast.reflection_type(t)))
            }
            TemplateIdRef::Value(e) => match self.ast.exprs.get(e).kind() {
                ExprKind::DeclRefExpr => {
                    let d = self.ast.exprs.decl(e);
                    self.reflectee_of_decl(d).ok_or(Some("the specialization has no reflection"))
                }
                ExprKind::ConceptSpecializationExpr => match self.ast.exprs.satisfied(e) {
                    Some(ok) => {
                        Ok(Reflectee::Value(self.int_const(QualType::BOOL, i128::from(ok))))
                    }
                    None => Err(Some("the concept-id is dependent")),
                },
                _ => Err(None),
            },
            TemplateIdRef::Overload(ule) => match self.single_template_spec(ule) {
                // [meta.reflection.substitute]/2: not a function whose type
                // contains an undeduced placeholder type (its body is not
                // instantiated to deduce it; as GCC).
                Some((spec, _))
                    if self
                        .ast
                        .decls
                        .get(spec)
                        .ty()
                        .is_some_and(|t| self.contains_undeduced_auto(t)) =>
                {
                    Err(Some("the specialization's type contains an undeduced placeholder type"))
                }
                Some((spec, _)) => {
                    self.reflectee_of_decl(spec).ok_or(Some("the specialization has no reflection"))
                }
                None => {
                    Err(Some("the arguments do not name a unique function template specialization"))
                }
            },
        }
    }
}

/// The reason category of a substitution that failed with `d` (the
/// first error the SFINAE frame captured).
fn substitution_reason(d: Option<&Diag>) -> String {
    let Some(d) = d else { return "substitution failure".to_owned() };
    let category = match d.id() {
        Some(DiagId::sema_template_arg_list_different_arity) => {
            "wrong number of template arguments"
        }
        Some(
            DiagId::sema_template_arg_must_be_type
            | DiagId::sema_template_arg_must_be_type_suggest
            | DiagId::sema_template_arg_must_be_expr
            | DiagId::sema_template_arg_must_be_template
            | DiagId::sema_template_arg_not_valid_template
            | DiagId::sema_template_arg_template_params_mismatch,
        ) => "template argument of the wrong kind",
        Some(DiagId::sema_template_constraints_not_satisfied) => "constraints not satisfied",
        _ => "substitution failure",
    };
    format!("{category} ({})", d.message())
}
