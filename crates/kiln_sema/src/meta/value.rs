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
            ValueOp::Extract => self.extract(info_arg(args, 0)?, ret, loc)?,
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
            ValueOp::ReflectConstantString => {
                MetaOut::Info(self.reflect_constant_elems(arg(0)?, true, ctx, oven)?)
            }
            ValueOp::ReflectConstantArray => {
                MetaOut::Info(self.reflect_constant_elems(arg(0)?, false, ctx, oven)?)
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
    #[expect(clippy::unnecessary_wraps, reason = "meta_shape's signature")]
    pub(super) fn value_shape(
        &self,
        v: ValueOp,
        param: Option<usize>,
    ) -> Result<Option<Shape>, String> {
        Ok(match (v, param) {
            (ValueOp::Extract, None) => Some(Shape::Whole),
            (ValueOp::ReflectConstantString | ValueOp::ReflectConstantArray, Some(0)) => {
                Some(Shape::Elems)
            }
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
        self.push_const(Scalar::Pointer { ty, base, offset, designated: None })
    }

    /// Why frozen value `cv` is no template argument ([temp.arg.nontype]/6):
    /// a pointer or reference into a string literal, a temporary, a
    /// `typeid` result or a label, at the top level or in a subobject.
    fn bad_tparg_base(&self, cv: ConstValueId) -> Option<&'static str> {
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
                    PtrBase::Label { .. } => return Some("a label"),
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
                format!(
                    "{what} refers to {why}, which is not a valid constant template argument"
                ),
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
        self.check_tparg(cv, "the value", loc)?;
        let r = if self.ast.types.as_record(t).is_some() {
            let ct = self.ast.types.qualify(t, Quals::CONST);
            let obj = self.builder().template_param_object(ct, cv);
            Reflectee::Object(self.object_ptr(t, PtrBase::Decl(obj), 0)?)
        } else {
            Reflectee::Value(cv)
        };
        Ok(self.ast.reflections.intern(r))
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
            MetaError::note(loc, "the argument does not designate an object of static storage duration")
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
        Ok(self.ast.reflections.intern(Reflectee::Object(cv)))
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
                let d = self.ast.first(d);
                Ok(self.ast.reflections.intern(Reflectee::Decl(d)))
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
        } else if self.ast.types.is_array(e) || !self.is_structural(e) {
            return fail(loc, format!("'{}' is not a structural type", self.ty_str(e)));
        }
        let m = oven.meta_elems_len(r, string, ctx)?;
        if m == 0 && !string {
            // libstdc++ reflects an `array<T, 0>` here (WP-479 follow-up).
            return fail(loc, "kiln: 'reflect_constant_array' of an empty range is not supported");
        }
        let arr = self.ast.types.constant_array(e, m + u64::from(string), None);
        let cv = self.bridge(|b| oven.meta_freeze_elems(b, (r, m), arr, string, ctx))?;
        self.check_tparg(cv, "an element", loc)?;
        let p = self.object_ptr(arr, PtrBase::Const(cv), 0)?;
        Ok(self.ast.reflections.intern(Reflectee::Object(p)))
    }

    /// `extract<T>(r)` ([meta.reflection.extract]), `t` being `T`.
    fn extract(&mut self, r: ReflectionId, t: QualType, loc: Loc) -> Result<MetaOut, MetaError> {
        let tc = self.ast.types.canonical(t);
        match self.ast.reflections.get(r) {
            Reflectee::Value(cv) => self.extract_value(r, cv, tc, loc),
            Reflectee::Object(p) => self.extract_object(r, p, tc, loc),
            Reflectee::Decl(d) => match self.kind_of(d) {
                DeclKind::Var | DeclKind::VarTemplateSpecialization => {
                    let Some(vt) = self.ast.decls.get(d).ty() else {
                        return Err(MetaError::invalid());
                    };
                    if self.ast.types.is_reference(vt) {
                        return fail(
                            loc,
                            format!(
                                "kiln: extracting reference variable '{}' is not supported",
                                self.reflection_str(r)
                            ),
                        );
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
                DeclKind::CXXMethod if self.is_static_method(d) => {
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
        let cv_ok = |q: QualType| !konst || q.quals().is_const();
        if t.is_reference(tc) {
            let tt = t.pointee(tc).map_or(QualType::ERROR, |q| t.canonical(q));
            if tt.without_quals() == u && cv_ok(tt) {
                return Ok(MetaOut::Frozen(p));
            }
            return self.mismatch(r, tc, loc);
        }
        if t.is_pointer(tc) && t.is_array(u) {
            let e = t.element(u).map_or(QualType::ERROR, |q| t.canonical(q));
            let pt = t.pointee(tc).map_or(QualType::ERROR, |q| t.canonical(q));
            if e.without_quals() == pt.without_quals()
                && pt.quals().contains(e.quals())
                && cv_ok(pt)
            {
                let ep = self.object_ptr(e, base, offset)?;
                return Ok(MetaOut::Frozen(ep));
            }
            return self.mismatch(r, tc, loc);
        }
        if tc.without_quals() == u && !t.is_array(u) {
            return Ok(MetaOut::Deref(p));
        }
        self.mismatch(r, tc, loc)
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
        let t = &self.ast.types;
        let ok = (t.is_pointer(tc) || t.is_reference(tc))
            && t.pointee(tc).is_some_and(|q| t.canonical(q) == ft);
        if !ok {
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
        let class_ty = QualType::from(self.ast.types.record_type(cls));
        let t = &self.ast.types;
        let ok = match t.canonical_view(tc) {
            TypeView::MemberPointer { pointee, class } => {
                t.canonical(pointee) == t.canonical(mt)
                    && t.canonical(QualType::from(class)) == t.canonical(class_ty)
            }
            _ => false,
        };
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
