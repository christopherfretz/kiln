//! The sema half of the value family (spec/reflection-values.md;
//! [meta.reflection.extract], [meta.reflection.result], P3491; WP-479):
//! `extract<T>`, `reflect_constant`, `reflect_object`, `reflect_function`,
//! `reflect_constant_string`, `reflect_constant_array` and
//! `is_string_literal`'s builtin. The Oven freezes an operand into the
//! constant store (content-interned), so equal values give one store value
//! and one reflection; sema checks the [temp.arg.nontype] rules on it and
//! picks the reflectee. `T` is read off the call's specialization.

use kiln_ast::consts::{ConstValue, IntBits, MemberKind, PtrBase, Scalar};
use kiln_ast::decl::DeclKind;
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::{BuiltinType, Quals, TypeView};
use kiln_ast::{ConstValueId, DeclId, QualType, ReflectionId};
use kiln_oven::{MetaCtx, MetaError, MetaOut, Oven, Shape, ValueOp, info_arg};
use kiln_source::Loc;

use super::fail;
use crate::Sema;

/// How many relocations [`Sema::bad_tparg_base`] follows at most (a bound
/// on a malformed value).
const MAX_RELOCS: usize = 1 << 16;

impl Sema<'_, '_> {
    pub(super) fn meta_value(
        &mut self,
        v: ValueOp,
        args: &[u128],
        out: &mut MetaOut,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<(), MetaError> {
        let loc = ctx.loc;
        let (ret, p0) = self.value_sig(ctx.callee).ok_or_else(MetaError::invalid)?;
        let arg = |i: usize| args.get(i).copied().ok_or_else(MetaError::invalid);
        let p0 = || p0.ok_or_else(MetaError::invalid);
        *out = match v {
            ValueOp::Extract => {
                let r = info_arg(args, 0)?;
                self.use_reflected_var(r, loc, oven);
                let out = self.extract(r, ret, loc)?;
                if let Some(f) = self.extracted_function(r, &out) {
                    self.use_reflected_function(f, loc, oven);
                }
                out
            }
            ValueOp::ReflectConstant | ValueOp::ReflectConstantInfo => {
                let t = self.referenced(p0()?);
                MetaOut::Info(self.reflect_constant(arg(0)?, t, loc, oven)?)
            }
            ValueOp::ReflectObject => {
                let t = self.referenced(p0()?);
                MetaOut::Info(self.reflect_object(arg(0)?, t, loc, oven)?)
            }
            ValueOp::ReflectFunction => {
                let t = self.referenced(p0()?);
                MetaOut::Info(self.reflect_function(arg(0)?, t, loc, oven)?)
            }
            ValueOp::ReflectConstantString | ValueOp::ReflectConstantStringStd => {
                MetaOut::Info(self.reflect_constant_elems(arg(0)?, true, ctx, oven)?)
            }
            ValueOp::ReflectConstantArray | ValueOp::ReflectConstantArrayStd => {
                MetaOut::Info(self.reflect_constant_elems(arg(0)?, false, ctx, oven)?)
            }
            ValueOp::ExcCvtFromUtf8 | ValueOp::ExcCvtToUtf8 => {
                let r = arg(0)?;
                let m = (r >> 64) as u64;
                let s = oven
                    .meta_elems_bytes(r, m, ctx)?
                    .and_then(|b| String::from_utf8(b).ok())
                    .unwrap_or_default();
                let u8 = v == ValueOp::ExcCvtToUtf8;
                let Some(cv) = self.frozen_string(&s, u8) else {
                    return fail(loc, "kiln: message too large for a constant");
                };
                MetaOut::Str(cv, s.len() as u64)
            }
            ValueOp::IsStringLiteral => {
                MetaOut::Scalar(u128::from(oven.meta_is_string_literal(arg(0)?)))
            }
        };
        Ok(())
    }

    /// The [`Shape`] of value op `v`'s class-typed result (`param: None`)
    /// or argument `param` ([`Sema::meta_shape`]): `extract<T>` of a
    /// class `T` is copied whole; the range of `reflect_constant_string`
    /// and `reflect_constant_array` is any contiguous range.
    #[expect(
        clippy::unnecessary_wraps,
        clippy::unused_self,
        reason = "meta_shape's per-family signature"
    )]
    pub(super) fn value_shape(
        &self,
        v: ValueOp,
        param: Option<usize>,
    ) -> Result<Option<Shape>, String> {
        Ok(match (v, param) {
            (ValueOp::Extract, None) => Some(Shape::Whole),
            (
                ValueOp::ReflectConstantString
                | ValueOp::ReflectConstantArray
                | ValueOp::ReflectConstantStringStd
                | ValueOp::ReflectConstantArrayStd
                | ValueOp::ExcCvtFromUtf8
                | ValueOp::ExcCvtToUtf8,
                Some(0),
            ) => Some(Shape::Elems),
            _ => None,
        })
    }

    /// The result type and first parameter type of intrinsic `f` (a
    /// function template specialization: its `T` substituted).
    fn value_sig(&self, f: Option<DeclId>) -> Option<(QualType, Option<QualType>)> {
        let q = self.ast.decls.get(f?).ty()?;
        let p = self.ast.types.proto(q)?;
        Some((p.ret(), self.ast.types.params(p).first().copied()))
    }

    /// `q` with a reference stripped, canonical and cv-unqualified.
    fn referenced(&self, q: QualType) -> QualType {
        let t = &self.ast.types;
        let c = t.canonical(q);
        let c = if t.is_reference(c) { t.pointee(c).unwrap_or(c) } else { c };
        t.canonical(c).without_quals()
    }

    fn unqual(&self, q: QualType) -> QualType {
        self.ast.types.canonical(q).without_quals()
    }

    fn push_const(&mut self, s: Scalar<'_>) -> Result<ConstValueId, MetaError> {
        self.ast.consts.push_scalar(s).map_err(|_| MetaError::memory())
    }

    /// The interned pointer of an object reflection: to `base` + `offset`,
    /// of type pointer to `obj` cv-unqualified (one pointer, so one
    /// reflection, per object whatever cv the operand had).
    fn object_ptr(
        &mut self,
        obj: QualType,
        base: PtrBase,
        offset: u64,
    ) -> Result<ConstValueId, MetaError> {
        let obj = self.unqual(obj);
        let ty = self.ast.types.pointer_to(obj);
        self.push_const(Scalar::Pointer { ty, base, offset, designated: None, past: false })
    }

    /// Why frozen value `cv` is no template argument ([temp.arg.nontype]/6):
    /// a pointer or reference into a string literal, a temporary, a
    /// `typeid` result or a label, at the top level or in a subobject.
    pub(crate) fn bad_tparg_base(&self, cv: ConstValueId) -> Option<&'static str> {
        let mut todo = vec![cv];
        let mut seen = 0usize;
        while let Some(cv) = todo.pop() {
            seen += 1;
            if seen > MAX_RELOCS {
                return None;
            }
            match self.ast.consts.get(cv) {
                ConstValue::Scalar(Scalar::Pointer { base, .. }) => match base {
                    PtrBase::StringLit { .. } => return Some("a string literal"),
                    PtrBase::Temporary(_) => return Some("a temporary object"),
                    PtrBase::TypeInfo(_) => return Some("a 'typeid' result"),
                    PtrBase::Label { .. } | PtrBase::LabelDiff { .. } => return Some("a label"),
                    PtrBase::Absolute => return Some("an integer cast to a pointer"),
                    PtrBase::Decl(_) | PtrBase::Function(_) | PtrBase::Const(_) => {}
                },
                ConstValue::Scalar(_) => {}
                ConstValue::Aggregate(a) => todo.extend(a.relocs.iter().map(|r| r.value)),
            }
        }
        None
    }

    fn check_tparg(&self, cv: ConstValueId, what: &str, loc: Loc) -> Result<(), MetaError> {
        match self.bad_tparg_base(cv) {
            Some(why) => fail(
                loc,
                format!("{what} refers to {why}, which is not a valid constant template argument"),
            ),
            None => Ok(()),
        }
    }

    /// `reflect_constant(v)` of type `t` ([meta.reflection.result]/2): a
    /// class value is the template parameter object holding it (an object
    /// reflection), anything else the value itself.
    fn reflect_constant(
        &mut self,
        v: u128,
        t: QualType,
        loc: Loc,
        oven: &mut Oven,
    ) -> Result<ReflectionId, MetaError> {
        let ty = &self.ast.types;
        if ty.is_array(t) || ty.is_function(t) || !self.is_structural(t) {
            return fail(loc, format!("'{}' is not a structural type", self.ty_str(t)));
        }
        let cv = self.bridge(|b| oven.meta_freeze(b, v, t, loc))?;
        self.constant_reflection(cv, loc)
    }

    /// The reflection of annotation `a`'s underlying constant
    /// ([dcl.attr.annotation]/2): `constant_of(a)` (P3394R4). A class
    /// value is its template parameter object, as `reflect_constant`
    /// makes it.
    pub(super) fn annotation_constant(
        &mut self,
        a: kiln_ast::AnnId,
        loc: Loc,
    ) -> Result<ReflectionId, MetaError> {
        let Some(cv) = self.ast.annots.get(a).value else {
            return Err(MetaError::invalid());
        };
        self.constant_reflection(cv, loc)
    }

    /// The reflection of constant `cv` ([meta.reflection.result]/2
    /// `reflect_constant`; [meta.reflection.queries] `constant_of`): a
    /// class value is the template parameter object holding it (an
    /// object reflection), an array value the `reflect_constant_array`
    /// object holding its elements (P3491R3), anything else the value
    /// itself. The image is retyped cv-unqualified first, so one value
    /// is one object whatever spelling of its type froze it.
    pub(super) fn constant_reflection(
        &mut self,
        cv: ConstValueId,
        loc: Loc,
    ) -> Result<ReflectionId, MetaError> {
        let t = self.unqual(self.ast.consts.ty(cv));
        let r = if self.ast.types.is_array(t) {
            let ty = &mut self.ast.types;
            let e = ty.element(t).map_or(QualType::ERROR, |q| ty.canonical(q));
            let Some(n) = ty.array_size(t) else { return Err(MetaError::invalid()) };
            let at = ty.constant_array(e.without_quals(), n, None);
            let cv = self.retyped(cv, at)?;
            self.check_tparg(cv, "an element", loc)?;
            Reflectee::Object(self.object_ptr(at, PtrBase::Const(cv), 0)?)
        } else if self.ast.types.as_record(t).is_some() {
            let cv = self.retyped(cv, t)?;
            self.check_tparg(cv, "the value", loc)?;
            let ct = self.ast.types.qualify(t, Quals::CONST);
            let obj = self.builder().template_param_object(ct, cv);
            Reflectee::Object(self.object_ptr(t, PtrBase::Decl(obj), 0)?)
        } else {
            self.check_tparg(cv, "the value", loc)?;
            Reflectee::Value(cv)
        };
        self.intern_reflection(r, loc).map_err(MetaError::from)
    }

    /// Aggregate constant `cv` typed `ty` (re-pushed when its image has
    /// another spelling); a scalar as is (it is typed cv-unqualified).
    fn retyped(&mut self, cv: ConstValueId, ty: QualType) -> Result<ConstValueId, MetaError> {
        let Some(img) = self.ast.consts.aggregate(cv) else { return Ok(cv) };
        if img.ty == ty {
            return Ok(cv);
        }
        let (bytes, relocs) = (img.bytes.to_vec(), img.relocs.to_vec());
        let (padding, unions) = (img.padding.to_vec(), img.unions.to_vec());
        let img = kiln_ast::consts::ByteImage {
            ty,
            bytes: &bytes,
            relocs: &relocs,
            padding: &padding,
            unions: &unions,
        };
        self.ast.consts.push_aggregate(img).map_err(|_| MetaError::memory())
    }

    /// `reflect_object(o)` of type `t` ([meta.reflection.result]/5): `p`
    /// is the object's address.
    fn reflect_object(
        &mut self,
        p: u128,
        t: QualType,
        loc: Loc,
        oven: &mut Oven,
    ) -> Result<ReflectionId, MetaError> {
        if self.ast.types.is_function(t) {
            return fail(loc, format!("'{}' is not an object type", self.ty_str(t)));
        }
        let not_static = || {
            MetaError::note(
                loc,
                "the argument does not designate an object of static storage duration",
            )
        };
        if p == 0 {
            return Err(not_static());
        }
        let pt = self.ast.types.pointer_to(t);
        let cv = self.bridge(|b| oven.meta_freeze(b, p, pt, loc))?;
        self.check_tparg(cv, "the object", loc)?;
        let Some(Scalar::Pointer { base, offset, .. }) = self.ast.consts.scalar(cv) else {
            return Err(not_static());
        };
        let cv = self.object_ptr(t, base, offset)?;
        self.intern_reflection(Reflectee::Object(cv), loc).map_err(MetaError::from)
    }

    /// `reflect_function(f)` of function type `t`
    /// ([meta.reflection.result]/7): `^^f` (an immediate function is no
    /// template argument).
    fn reflect_function(
        &mut self,
        p: u128,
        t: QualType,
        loc: Loc,
        oven: &mut Oven,
    ) -> Result<ReflectionId, MetaError> {
        if !self.ast.types.is_function(t) {
            return fail(loc, format!("'{}' is not a function type", self.ty_str(t)));
        }
        let pt = self.ast.types.pointer_to(t);
        let cv = self.bridge(|b| oven.meta_freeze(b, p, pt, loc))?;
        match self.ast.consts.scalar(cv) {
            Some(Scalar::Pointer { base: PtrBase::Function(d), .. }) => {
                let d = self.entity_decl(d);
                self.intern_reflection(Reflectee::Decl(d), loc).map_err(MetaError::from)
            }
            _ => fail(loc, "the argument does not designate a function"),
        }
    }

    /// `reflect_constant_string(r)` / `reflect_constant_array(r)` (P3491):
    /// the static-storage array holding `r`'s elements (a string's with a
    /// terminator), one per content.
    fn reflect_constant_elems(
        &mut self,
        r: u128,
        string: bool,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<ReflectionId, MetaError> {
        let loc = ctx.loc;
        let e = self.unqual(oven.meta_elem(ctx));
        if string {
            let char_like = matches!(
                self.ast.types.builtin(e),
                Some(
                    BuiltinType::Char
                        | BuiltinType::WChar
                        | BuiltinType::Char8
                        | BuiltinType::Char16
                        | BuiltinType::Char32
                )
            );
            if !char_like {
                return fail(
                    loc,
                    format!("'{}' is not a character type of a string", self.ty_str(e)),
                );
            }
        } else {
            // An array element is reflect_constant_array of it: the
            // structural requirement is on remove_all_extents_t<U>.
            let mut t = e;
            while self.ast.types.is_array(t) {
                match self.ast.types.element(t) {
                    Some(x) if self.ast.types.array_size(t).is_some() => t = x,
                    _ => {
                        return fail(loc, format!("'{}' is not a structural type", self.ty_str(e)));
                    }
                }
            }
            if !self.is_structural(t) {
                return fail(loc, format!("'{}' is not a structural type", self.ty_str(t)));
            }
        }
        let m = oven.meta_elems_len(r, string, ctx)?;
        if m == 0 && !string {
            return self.reflect_empty_array(e, ctx, oven);
        }
        let arr = self.ast.types.constant_array(e, m + u64::from(string), None);
        let cv = self.bridge(|b| oven.meta_freeze_elems(b, (r, m), arr, string, ctx))?;
        self.check_tparg(cv, "an element", loc)?;
        let p = self.object_ptr(arr, PtrBase::Const(cv), 0)?;
        self.intern_reflection(Reflectee::Object(p), loc).map_err(MetaError::from)
    }

    /// `reflect_constant_array` of an empty range of `T`s (P3491R3
    /// [meta.reflection.array]): the template parameter object of type
    /// `const std::array<T, 0>`, value-initialized. kiln reads it when
    /// `std::array<T, 0>` holds no data (libstdc++'s does).
    fn reflect_empty_array(
        &mut self,
        e: QualType,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<ReflectionId, MetaError> {
        let loc = ctx.loc;
        let unsupported = |s: &Self| {
            fail(
                loc,
                format!(
                    "kiln: 'reflect_constant_array' of an empty range needs 'std::array<{}, 0>' to be \
                     declared and to hold no data",
                    s.ty_str(e)
                ),
            )
        };
        let Some((_, tmpl)) = self.std_class_template("array") else {
            return unsupported(self);
        };
        // Instantiating `array<T, 0>` may evaluate: with the Oven lent back.
        let t = self.lend_oven(oven, |s| {
            let name = s.template_name_for(tmpl);
            let args = [kiln_ast::types::TemplateArg::ty(e), s.size_t_arg(0)];
            match s.act_template_id(name, &args, loc, loc) {
                Ok(crate::templ::actions::TemplateIdRef::Type(t))
                    if s.require_complete_silent(t, loc) =>
                {
                    Some(t)
                }
                _ => None,
            }
        });
        let Some(t) = t else {
            return unsupported(self);
        };
        let t = self.unqual(t);
        let Some(cv) = self.bridge(|b| oven.meta_freeze_blank(b, t))? else {
            return unsupported(self);
        };
        let p = self.object_ptr(t, PtrBase::Const(cv), 0)?;
        self.intern_reflection(Reflectee::Object(p), loc).map_err(MetaError::from)
    }

    /// `extract<T>(r)` ([meta.reflection.extract]), `t` being `T`.
    fn extract(&mut self, r: ReflectionId, t: QualType, loc: Loc) -> Result<MetaOut, MetaError> {
        let tc = self.ast.types.canonical(t);
        match self.ast.reflections.get(r) {
            Reflectee::Value(cv) => self.extract_value(r, cv, tc, loc),
            // An annotation's underlying constant ([meta.reflection.extract]
            // via `constant_of`, P3394R4).
            Reflectee::Annotation(a) => {
                let c = self.annotation_constant(a, loc)?;
                self.extract(c, t, loc)
            }
            Reflectee::Object(p) => self.extract_object(r, p, tc, loc),
            Reflectee::Decl(d) => match self.kind_of(d) {
                DeclKind::Var | DeclKind::VarTemplateSpecialization => {
                    let Some(vt) = self.ast.decls.get(d).ty() else {
                        return Err(MetaError::invalid());
                    };
                    if self.ast.types.is_reference(vt) {
                        return self.extract_through_ref(r, d, vt, tc, loc);
                    }
                    if !self.has_static_storage(d) {
                        return fail(
                            loc,
                            format!(
                                "'{}' is not a variable of static storage duration",
                                self.reflection_str(r)
                            ),
                        );
                    }
                    let first = self.ast.first(d);
                    let p = self.object_ptr(vt, PtrBase::Decl(first), 0)?;
                    let konst = self.ast.types.canonical(vt).quals().is_const();
                    self.extract_place(r, p, konst, tc, loc)
                }
                DeclKind::Function => self.extract_function(r, d, tc, loc),
                DeclKind::CXXMethod if self.is_static_method(d) || self.has_explicit_object(d) => {
                    self.extract_function(r, d, tc, loc)
                }
                DeclKind::CXXMethod | DeclKind::CXXConversion | DeclKind::Field => {
                    self.extract_member(r, d, tc, loc)
                }
                DeclKind::EnumConstant => self.extract_enumerator(r, d, tc, loc),
                _ => self.not_extractable(r, loc),
            },
            _ => self.not_extractable(r, loc),
        }
    }

    /// `extract<T>` of reference variable `d` of type `vt`: of the object
    /// it refers to, its initializer being a constant.
    fn extract_through_ref(
        &mut self,
        r: ReflectionId,
        d: DeclId,
        vt: QualType,
        tc: QualType,
        loc: Loc,
    ) -> Result<MetaOut, MetaError> {
        let v = match self.ast.decls.var(d).map(|v| v.eval.state()) {
            Some(kiln_ast::decl::VarEvalState::Value(v)) => self.ast.consts.scalar(v),
            _ => None,
        };
        let Some(Scalar::Pointer { base, offset, .. }) = v else {
            return fail(
                loc,
                format!(
                    "kiln: extracting through reference '{}' needs it to be constexpr",
                    self.reflection_str(r)
                ),
            );
        };
        let obj = self.referenced(vt);
        let konst = self.ast.types.canonical(obj).quals().is_const();
        let p = self.object_ptr(obj, base, offset)?;
        self.extract_place(r, p, konst, tc, loc)
    }

    fn not_extractable<T>(&self, r: ReflectionId, loc: Loc) -> Result<T, MetaError> {
        fail(
            loc,
            format!(
                "'{}' does not reflect a value, object, variable, function, enumerator or \
                 non-static member",
                self.reflection_str(r)
            ),
        )
    }

    fn mismatch<T>(&self, r: ReflectionId, t: QualType, loc: Loc) -> Result<T, MetaError> {
        fail(
            loc,
            format!("'{}' cannot be extracted as a '{}'", self.reflection_str(r), self.ty_str(t)),
        )
    }

    /// Whether pointer types `from` and `to` (canonical) point to the same
    /// type, `to`'s pointee at least as cv-qualified (the qualification
    /// conversion `extract` allows on a pointer value).
    fn ptr_convertible(&self, from: QualType, to: QualType) -> bool {
        let t = &self.ast.types;
        let (Some(f), Some(g)) = (t.pointee(from), t.pointee(to)) else { return false };
        let (f, g) = (t.canonical(f), t.canonical(g));
        f.without_quals() == g.without_quals() && g.quals().contains(f.quals())
    }

    fn extract_value(
        &mut self,
        r: ReflectionId,
        cv: ConstValueId,
        tc: QualType,
        loc: Loc,
    ) -> Result<MetaOut, MetaError> {
        let u = self.ast.types.canonical(self.ast.consts.ty(cv));
        if self.ast.types.is_reference(tc) {
            return fail(
                loc,
                format!(
                    "'{}' is a value, not an object: it cannot be extracted as a reference",
                    self.reflection_str(r)
                ),
            );
        }
        let same = tc.without_quals() == u.without_quals();
        let ptr = self.ast.types.is_pointer(tc)
            && self.ast.types.is_pointer(u)
            && self.ptr_convertible(u, tc);
        if same || ptr { Ok(MetaOut::Frozen(cv)) } else { self.mismatch(r, tc, loc) }
    }

    fn extract_object(
        &mut self,
        r: ReflectionId,
        p: ConstValueId,
        tc: QualType,
        loc: Loc,
    ) -> Result<MetaOut, MetaError> {
        let Some(Scalar::Pointer { base, .. }) = self.ast.consts.scalar(p) else {
            return Err(MetaError::invalid());
        };
        // A template parameter object and a `reflect_constant_array`
        // object are const.
        let konst = match base {
            PtrBase::Decl(d) => {
                self.kind_of(d) == DeclKind::TemplateParamObject
                    || self
                        .ast
                        .decls
                        .get(d)
                        .ty()
                        .is_some_and(|q| self.ast.types.canonical(q).quals().is_const())
            }
            PtrBase::Const(_) => true,
            _ => false,
        };
        self.extract_place(r, p, konst, tc, loc)
    }

    /// `extract<T>` of the object pointer `p` designates (`konst`: the
    /// object is const): a reference to it, a pointer to its first element
    /// (an array), or its value.
    fn extract_place(
        &mut self,
        r: ReflectionId,
        p: ConstValueId,
        konst: bool,
        tc: QualType,
        loc: Loc,
    ) -> Result<MetaOut, MetaError> {
        let Some(Scalar::Pointer { ty: pty, base, offset, .. }) = self.ast.consts.scalar(p) else {
            return Err(MetaError::invalid());
        };
        let t = &self.ast.types;
        let u = t.pointee(pty).map_or(QualType::ERROR, |q| t.canonical(q));
        if t.is_reference(tc) {
            let tt = t.pointee(tc).map_or(QualType::ERROR, |q| t.canonical(q));
            if self.qual_compatible(u, tt, konst) {
                return Ok(MetaOut::Frozen(p));
            }
            return self.mismatch(r, tc, loc);
        }
        if t.is_pointer(tc) && t.is_array(u) {
            let e = t.element(u).map_or(QualType::ERROR, |q| t.canonical(q));
            let pt = t.pointee(tc).map_or(QualType::ERROR, |q| t.canonical(q));
            if e.without_quals() == pt.without_quals()
                && pt.quals().contains(e.quals())
                && pt.quals().is_const()
            {
                // `extract-value<T>(constant_of(r))`: the pointer is into
                // `reflect_constant_array` of the array's value, not into
                // the array itself ([meta.reflection.queries] constant_of).
                let base = match base {
                    PtrBase::Const(_) => base,
                    _ => PtrBase::Const(self.array_constant(r, base, offset, u, loc)?),
                };
                let ep = self.object_ptr(e, base, 0)?;
                return Ok(MetaOut::Frozen(ep));
            }
            return self.mismatch(r, tc, loc);
        }
        if tc.without_quals() == u && !t.is_array(u) {
            return Ok(MetaOut::Deref(p));
        }
        self.mismatch(r, tc, loc)
    }

    /// The content-interned `reflect_constant_array` object holding the
    /// value of the array of type `u` at `base` + `offset`: a whole
    /// variable usable in constant expressions.
    fn array_constant(
        &mut self,
        r: ReflectionId,
        base: PtrBase,
        offset: u64,
        u: QualType,
        loc: Loc,
    ) -> Result<ConstValueId, MetaError> {
        let v = match base {
            PtrBase::Decl(d) if offset == 0 => {
                match self.ast.decls.var(d).map(|v| v.eval.state()) {
                    Some(kiln_ast::decl::VarEvalState::Value(v)) => Some(v),
                    _ => None,
                }
            }
            _ => None,
        };
        let Some(img_cv) = v.filter(|&v| self.ast.consts.aggregate(v).is_some()) else {
            return fail(
                loc,
                format!(
                    "kiln: the value of '{}' is not available to 'constant_of'",
                    self.reflection_str(r)
                ),
            );
        };
        let t = &mut self.ast.types;
        let (e, n) = (t.element(u).map_or(QualType::ERROR, |q| t.canonical(q)), t.array_size(u));
        let Some(n) = n else { return Err(MetaError::invalid()) };
        let ty = t.constant_array(e.without_quals(), n, None);
        let cv = self.retyped(img_cv, ty)?;
        self.check_tparg(cv, "an element", loc)?;
        Ok(cv)
    }

    /// Whether a reference to `t` binds to an object of type `u` (const
    /// when `konst`) by at most a qualification conversion
    /// ([meta.reflection.extract]/5: `U(*)[]` converts to `T(*)[]`); array
    /// bounds match level by level, the cv-qualifiers apply to the
    /// innermost element.
    fn qual_compatible(&self, u: QualType, t: QualType, konst: bool) -> bool {
        let ty = &self.ast.types;
        let (mut u, mut t) = (ty.canonical(u), ty.canonical(t));
        while ty.is_array(u) || ty.is_array(t) {
            if !ty.is_array(u) || !ty.is_array(t) || ty.array_size(u) != ty.array_size(t) {
                return false;
            }
            let el = |q: QualType| ty.element(q).map_or(QualType::ERROR, |e| ty.canonical(e));
            (u, t) = (el(u), el(t));
        }
        u.without_quals() == t.without_quals()
            && t.quals().contains(u.quals())
            && (!konst || t.quals().is_const())
    }

    /// The function `extract` of declaration reflection `r` formed a
    /// pointer (or pointer to member) to, `out` being its result.
    fn extracted_function(&self, r: ReflectionId, out: &MetaOut) -> Option<DeclId> {
        if !matches!(self.ast.reflections.get(r), Reflectee::Decl(_)) {
            return None;
        }
        let MetaOut::Frozen(cv) = *out else { return None };
        match self.ast.consts.scalar(cv)? {
            Scalar::Pointer { base: PtrBase::Function(f), .. } => Some(f),
            Scalar::MemberPtr { member, kind: MemberKind::Function, .. } => Some(member),
            _ => None,
        }
    }

    /// `extract<T>` of a non-member (or static member) function: a
    /// pointer or reference to it.
    fn extract_function(
        &mut self,
        r: ReflectionId,
        d: DeclId,
        tc: QualType,
        loc: Loc,
    ) -> Result<MetaOut, MetaError> {
        let Some(ft) = self.ast.decls.get(d).ty() else { return Err(MetaError::invalid()) };
        let ft = self.unqual(ft);
        // `T` is `F*`, the function's type being `F` or `F noexcept`
        // ([meta.reflection.extract]/7.3); a reference `T` is
        // `extract-ref`'s, which takes only variables and objects.
        let t = &self.ast.types;
        let want = t.is_pointer(tc).then(|| t.pointee(tc).map(|q| t.canonical(q))).flatten();
        if self.ast.types.is_reference(tc) {
            return fail(
                loc,
                format!(
                    "'{}' is a function, not an object: it cannot be extracted as a reference",
                    self.reflection_str(r)
                ),
            );
        }
        if !want.is_some_and(|w| w == ft || self.same_but_noexcept(w, ft)) {
            return self.mismatch(r, tc, loc);
        }
        let first = self.ast.first(d);
        self.mark_used(first);
        let ty = self.ast.types.pointer_to(ft);
        let cv = self.push_const(Scalar::Pointer {
            ty,
            base: PtrBase::Function(first),
            offset: 0,
            designated: None,
            past: false,
        })?;
        Ok(MetaOut::Frozen(cv))
    }

    /// `extract<T>` of a non-static data member or implicit object member
    /// function of class `C`: the pointer to member `X C::*`.
    fn extract_member(
        &mut self,
        r: ReflectionId,
        d: DeclId,
        tc: QualType,
        loc: Loc,
    ) -> Result<MetaOut, MetaError> {
        if !self.is_implicit_object_member(d) {
            return self.mismatch(r, tc, loc);
        }
        let field = self.kind_of(d) == DeclKind::Field;
        if field && self.ast.decls.field(d).is_some_and(|f| f.bit_width.is_some()) {
            return fail(
                loc,
                format!("'{}' is a bit-field: it has no pointer to member", self.reflection_str(r)),
            );
        }
        let (Some(mt), Some(cls)) = (self.ast.decls.get(d).ty(), self.ast.decls.get(d).ctx())
        else {
            return Err(MetaError::invalid());
        };
        let c = self.ast.types.record_type(cls);
        let class_ty = QualType::from(c);
        if self.ast.types.is_reference(tc) {
            return fail(
                loc,
                format!(
                    "'{}' is a non-static member, not an object: it cannot be extracted as a \
                     reference",
                    self.reflection_str(r)
                ),
            );
        }
        let t = &self.ast.types;
        let target = match t.canonical_view(tc) {
            TypeView::MemberPointer { pointee, class }
                if t.canonical(QualType::from(class)) == t.canonical(class_ty) =>
            {
                Some(t.canonical(pointee))
            }
            _ => None,
        };
        let mt = self.ast.types.canonical(mt);
        // A data member's `X C::*` converts by a qualification conversion
        // (7.1); a member function's type is `F` or `F noexcept` (7.2).
        let ok = target.is_some_and(|w| {
            if field {
                let from = self.ast.types.member_pointer(mt, c);
                crate::conv::is_qualification_convertible(self, from, tc.without_quals())
            } else {
                w == mt || self.same_but_noexcept(w, mt)
            }
        });
        if !ok {
            return self.mismatch(r, tc, loc);
        }
        let first = self.ast.first(d);
        if !field {
            self.mark_used(first);
        }
        let kind = if field { MemberKind::Data } else { MemberKind::Function };
        let cv = self.push_const(Scalar::MemberPtr {
            ty: tc.without_quals(),
            member: first,
            kind,
            derived: false,
            path: &[],
        })?;
        Ok(MetaOut::Frozen(cv))
    }

    /// Whether function type `w` is non-throwing function type `f` without
    /// its `noexcept`.
    fn same_but_noexcept(&mut self, w: QualType, f: QualType) -> bool {
        crate::conv::drop_noexcept(self, f).is_some_and(|g| self.ast.types.same_type(g, w))
    }

    /// `extract<E>` of an enumerator of `E`: its value.
    fn extract_enumerator(
        &mut self,
        r: ReflectionId,
        d: DeclId,
        tc: QualType,
        loc: Loc,
    ) -> Result<MetaOut, MetaError> {
        let et = self.ast.decls.get(d).ty().map(|q| self.unqual(q));
        let Some(v) = self.ast.decls.enumerator(d).and_then(|x| x.value) else {
            return Err(MetaError::invalid());
        };
        if et != Some(tc.without_quals()) {
            return self.mismatch(r, tc, loc);
        }
        if self.unqual(self.ast.consts.ty(v)) == tc.without_quals() {
            return Ok(MetaOut::Frozen(v));
        }
        let Some(Scalar::Int { signed, bits, .. }) = self.ast.consts.scalar(v) else {
            return Err(MetaError::invalid());
        };
        let (width, words) = (bits.width(), bits.words().to_vec());
        let bits = IntBits::from_words(width, &words).ok_or_else(MetaError::invalid)?;
        let cv = self.push_const(Scalar::Int { ty: tc.without_quals(), signed, bits })?;
        Ok(MetaOut::Frozen(cv))
    }
}
