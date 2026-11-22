//! The sema half of the value family (spec/reflection-values.md;
//! [meta.reflection.extract], [meta.reflection.result], P3491; WP-479):
//! `extract<T>`, `reflect_constant`, `reflect_object`, `reflect_function`,
//! `reflect_constant_string`, `reflect_constant_array` and
//! `is_string_literal`'s builtin. The Oven freezes an operand into the
//! constant store (content-interned), so equal values give one store value
//! and one reflection; sema checks the [temp.arg.nontype] rules on it and
//! picks the reflectee. `T` is read off the call's specialization.

use kiln_ast::consts::{ConstValue, IntBits, MemberKind, PtrBase, Scalar};
use kiln_ast::decl::{DeclKind, DefinitionBits};
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::{BuiltinType, Quals, TypeView};
use kiln_ast::{ConstValueId, DeclId, QualType, ReflectionId};
use kiln_oven::{MetaCtx, MetaError, MetaOut, Note, Oven, Shape, ValueOp, info_arg};
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
                let out = self.extract(r, ret, ctx, oven)?;
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
    pub(super) fn value_sig(&self, f: Option<DeclId>) -> Option<(QualType, Option<QualType>)> {
        let q = self.ast.decls.get(f?).ty()?;
        let p = self.ast.types.proto(q)?;
        Some((p.ret(), self.ast.types.params(p).first().copied()))
    }

    /// `q` with a reference stripped, canonical and cv-unqualified.
    pub(super) fn referenced(&self, q: QualType) -> QualType {
        let t = &self.ast.types;
        let c = t.canonical(q);
        let c = if t.is_reference(c) { t.pointee(c).unwrap_or(c) } else { c };
        t.canonical(c).without_quals()
    }

    pub(super) fn unqual(&self, q: QualType) -> QualType {
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
        let cv = match self.ast.types.as_record(t) {
            Some(r) if !self.copies_trivially(r) => {
                self.lend_oven(oven, |s| s.tparam_object_value(cv, t, loc))?
            }
            _ => cv,
        };
        self.constant_reflection(cv, loc)
    }

    /// Whether class `r`'s copies are its bytes: its copy constructors
    /// are trivial (a trivial copy keeps template-argument equivalence).
    fn copies_trivially(&self, r: DeclId) -> bool {
        let bits = self.defn_bits(r);
        bits.contains(DefinitionBits::TRIVIAL_COPY_CTOR)
            && !bits.contains(DefinitionBits::NONTRIVIAL_COPY_CTOR)
    }

    /// The value of the template parameter object of `TCls<V>`
    /// ([meta.reflection.result] `reflect_constant`) for class value `cv`
    /// of type `t` ([temp.arg.nontype]/4): `constexpr T v = V;` copies
    /// `V`, the object is copied from `v`, and it must be
    /// template-argument-equivalent to `v`, whose value it has; else
    /// `TCls<V>` is not valid and the call throws. Evaluated nested
    /// (sema holds the Oven).
    fn tparam_object_value(
        &mut self,
        cv: ConstValueId,
        t: QualType,
        loc: Loc,
    ) -> Result<ConstValueId, MetaError> {
        let t = self.unqual(t);
        let v = self.tparam_copy(cv, t, loc)?;
        let p = self.tparam_copy(v, t, loc)?;
        if p != v {
            return fail(
                loc,
                format!(
                    "a template parameter object copied from a value of type '{}' is not \
                     template-argument-equivalent to it",
                    self.ty_str(t)
                ),
            );
        }
        Ok(v)
    }

    /// The value of `const T` copy-initialized from an object holding
    /// class value `cv` (an lvalue: that value's template parameter
    /// object).
    fn tparam_copy(
        &mut self,
        cv: ConstValueId,
        t: QualType,
        loc: Loc,
    ) -> Result<ConstValueId, MetaError> {
        let cv = self.retyped(cv, t)?;
        let ct = self.ast.types.qualify(t, Quals::CONST);
        let o = self.builder().template_param_object(ct, cv);
        let e = self.builder().decl_ref(
            o,
            ct,
            loc,
            None,
            kiln_ast::expr::NonOdrUse::None,
            kiln_ast::expr::ExprFlags::NONE,
        );
        let c = self.perform_init(ct, crate::init::Initializer::Expr(e), loc, false);
        if self.is_err_expr(c) || self.has_errors_in(c) {
            return Err(MetaError::invalid());
        }
        let c = self.act_on_constant_full_expr(c);
        self.eval_const(c, kiln_oven::EvalContext::converted(loc))
            .map_err(|f| MetaError::failed(f.notes))
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
        if let Some(why) = self.elems_type_error(e, string) {
            return fail(loc, why);
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

    /// Why element type `e` (cv-unqualified) cannot be
    /// `reflect_constant_string`'s (`string`) or `reflect_constant_array`'s
    /// (checked before any element is read).
    pub(super) fn elems_type_error(&self, e: QualType, string: bool) -> Option<String> {
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
            return (!char_like)
                .then(|| format!("'{}' is not a character type of a string", self.ty_str(e)));
        }
        // An array element is reflect_constant_array of it: the
        // structural requirement is on remove_all_extents_t<U>.
        let mut t = e;
        while self.ast.types.is_array(t) {
            match self.ast.types.element(t) {
                Some(x) if self.ast.types.array_size(t).is_some() => t = x,
                _ => return Some(format!("'{}' is not a structural type", self.ty_str(e))),
            }
        }
        (!self.is_structural(t)).then(|| format!("'{}' is not a structural type", self.ty_str(t)))
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
        let no_empty_array = |s: &Self| {
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
            return no_empty_array(self);
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
            return no_empty_array(self);
        };
        let t = self.unqual(t);
        let Some(cv) = self.bridge(|b| oven.meta_freeze_blank(b, t))? else {
            return no_empty_array(self);
        };
        let p = self.object_ptr(t, PtrBase::Const(cv), 0)?;
        self.intern_reflection(Reflectee::Object(p), loc).map_err(MetaError::from)
    }

    /// Marks the calls in progress impure when `d` is an automatic
    /// variable (or a parameter): what `extract` or `constant_of` of it
    /// gives depends on the running evaluation's frames (spec/oven.md
    /// "Reflected locals"), not on the reflection alone, so no call
    /// given the reflection as an argument is memoized (the memo key
    /// holds the reflection's bits only).
    pub(super) fn note_frame_read(&self, d: DeclId, oven: &mut Oven) {
        // A structured binding reads its decomposition (WP-2161).
        let d = match self.kind_of(d) {
            DeclKind::Binding => match self.ast.decls.binding_decomp(d) {
                Some(e) => e,
                None => return,
            },
            _ => d,
        };
        let local = self.kind_of(d) == DeclKind::ParmVar
            || (self.ast.decls.var(d).is_some() && !self.has_static_storage(d));
        if local {
            oven.mark_impure();
        }
    }

    /// `extract<T>(r)` ([meta.reflection.extract]), `t` being `T`.
    fn extract(
        &mut self,
        r: ReflectionId,
        t: QualType,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<MetaOut, MetaError> {
        let loc = ctx.loc;
        let tc = self.ast.types.canonical(t);
        if let Reflectee::Decl(d) = self.ast.reflections.get(r) {
            self.note_frame_read(d, oven);
        }
        match self.ast.reflections.get(r) {
            Reflectee::Value(cv) => self.extract_value(r, cv, tc, loc),
            // An annotation's underlying constant ([meta.reflection.extract]
            // via `constant_of`, P3394R4).
            Reflectee::Annotation(a) => {
                let c = self.annotation_constant(a, loc)?;
                self.extract(c, t, ctx, oven)
            }
            Reflectee::Object(p) => self.extract_object(r, p, tc, loc),
            Reflectee::Decl(d) => match self.kind_of(d) {
                DeclKind::Var | DeclKind::VarTemplateSpecialization => {
                    let Some(vt) = self.ast.decls.get(d).ty() else {
                        return Err(MetaError::invalid());
                    };
                    if self.ast.types.is_reference(vt) {
                        // A block-scope reference of a live frame of the
                        // evaluation: its referent there (WP-1515); else
                        // one usable in constant expressions, else the
                        // lifetime failure.
                        let local = !self.has_static_storage(d);
                        if local && oven.meta_ref_local(d).is_none() {
                            let usable = self
                                .var_init_value(d, oven)
                                .and_then(|v| self.ast.consts.scalar(v))
                                .is_some_and(|v| matches!(v, Scalar::Pointer { .. }));
                            if !usable {
                                return self.extract_local(r, d, (vt, tc), loc, oven);
                            }
                        } else if local {
                            return self.extract_local(r, d, (vt, tc), loc, oven);
                        }
                        return self.extract_through_ref(r, d, (vt, tc), loc, oven);
                    }
                    if !self.has_static_storage(d) {
                        // By value, `extract-value<U>(constant_of(r))`
                        // ([meta.reflection.extract]/12): a local the
                        // evaluation did not start (one of an enclosing
                        // function, or of an earlier evaluation) that is
                        // usable in constant expressions gives its
                        // constant, whatever its storage duration. One
                        // whose lifetime began in this evaluation is read
                        // live, or fails once its block ended (WP-1521).
                        let other = oven.meta_local(d).is_none() && !oven.meta_local_ended(d);
                        if other && !self.ast.types.is_reference(tc) {
                            if let Ok(c) = self.reflect_constant_of(r, ctx, oven) {
                                return self.extract_constant(r, c, t, ctx, oven);
                            }
                        } else if other {
                            // extract-ref (/5.3): usable in constant
                            // expressions suffices; the reference
                            // designates the variable's (const) object.
                            let vc = self.ast.types.canonical(vt);
                            if self.var_constant_value(d, vc, oven).is_some() {
                                let first = self.ast.first(d);
                                let p = self.object_ptr(vt, PtrBase::Decl(first), 0)?;
                                return self.extract_place(r, p, true, tc, loc);
                            }
                        }
                        return self.extract_local(r, d, (vt, tc), loc, oven);
                    }
                    // A pointer by value is `extract-value<T>(constant_of(r))`
                    // ([meta.reflection.extract]/12): the variable's value,
                    // converted as a constant is (`int *const` into
                    // `const int *`), never a read through its address.
                    let vc = self.ast.types.canonical(vt);
                    if self.ast.types.is_pointer(tc) && self.ast.types.is_pointer(vc) {
                        let c = self.reflect_constant_of(r, ctx, oven)?;
                        return self.extract_constant(r, c, t, ctx, oven);
                    }
                    let first = self.ast.first(d);
                    let p = self.object_ptr(vt, PtrBase::Decl(first), 0)?;
                    self.extract_place(r, p, vc.quals().is_const(), tc, loc)
                }
                DeclKind::ParmVar => {
                    let Some(vt) = self.ast.decls.get(d).ty() else {
                        return Err(MetaError::invalid());
                    };
                    self.extract_local(r, d, (vt, tc), loc, oven)
                }
                DeclKind::Function => self.extract_function(r, d, tc, loc),
                // An explicit-object conversion function, too, is a
                // function whose address is an ordinary pointer
                // ([expr.unary.op]/3; WP-1782).
                DeclKind::CXXMethod | DeclKind::CXXConversion
                    if self.is_static_method(d) || self.has_explicit_object(d) =>
                {
                    self.extract_function(r, d, tc, loc)
                }
                DeclKind::CXXMethod | DeclKind::CXXConversion | DeclKind::Field => {
                    self.extract_member(r, d, tc, loc)
                }
                DeclKind::EnumConstant => self.extract_enumerator(r, d, tc, loc),
                // A structured binding is neither a variable nor an object
                // (no reference extract), but by value it is
                // `extract-value<U>(constant_of(r))`
                // ([meta.reflection.extract]/12; GCC 16.1 rejects it).
                DeclKind::Binding if !self.ast.types.is_reference(tc) => {
                    // One of a decomposition of a live frame of this
                    // evaluation is read there, as an automatic variable
                    // is (`extract_local`), when `T` is its type; else
                    // through `constant_of`, which reads it there too
                    // (WP-2161).
                    if let Some(addr) = oven.meta_ref_local(d) {
                        let bt = self.ast.decls.get(d).ty().map(|q| self.ast.types.canonical(q));
                        if bt.is_some_and(|u| {
                            tc.without_quals() == u.without_quals() && !self.ast.types.is_array(u)
                        }) {
                            return Ok(MetaOut::Live { addr, by_ref: false });
                        }
                    }
                    let c = self.reflect_constant_of(r, ctx, oven)?;
                    self.extract_constant(r, c, t, ctx, oven)
                }
                _ => self.not_extractable(r, loc),
            },
            _ => self.not_extractable(r, loc),
        }
    }

    /// `extract-value<U>(c)` for `c`, the `constant_of` of `r`: what `r`
    /// names in the diagnostics.
    fn extract_constant(
        &mut self,
        r: ReflectionId,
        c: ReflectionId,
        t: QualType,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<MetaOut, MetaError> {
        let tc = self.ast.types.canonical(t);
        match self.ast.reflections.get(c) {
            Reflectee::Value(cv) => self.extract_value(r, cv, tc, ctx.loc),
            Reflectee::Object(p) => self.extract_object(r, p, tc, ctx.loc),
            _ => self.extract(c, t, ctx, oven),
        }
    }

    /// `extract<T>` of reference variable `d` of type `vt`: of the object
    /// it refers to, its initializer being a constant (a reference
    /// constant-initialized is usable in constant expressions without
    /// `constexpr`, [expr.const]/4; WP-1462).
    fn extract_through_ref(
        &mut self,
        r: ReflectionId,
        d: DeclId,
        (vt, tc): (QualType, QualType),
        loc: Loc,
        oven: &mut Oven,
    ) -> Result<MetaOut, MetaError> {
        let v = self.var_init_value(d, oven).and_then(|v| self.ast.consts.scalar(v));
        let Some(Scalar::Pointer { base, offset, .. }) = v else {
            return fail(
                loc,
                format!(
                    "reference '{}' is not usable in constant expressions",
                    self.reflection_str(r)
                ),
            );
        };
        let obj = self.referenced(vt);
        let konst = self.ast.types.canonical(obj).quals().is_const();
        let p = self.object_ptr(obj, base, offset)?;
        self.extract_place(r, p, konst, tc, loc)
    }

    /// `extract<T>` of automatic variable or parameter `d` of type `vt`
    /// (not a reference): its object, if its lifetime began within the
    /// running evaluation and has not ended ([meta.reflection.extract],
    /// [expr.const]/10.9; the Oven finds it in a live frame,
    /// [`Oven::meta_local`]); a reference to it, or its value.
    fn extract_local(
        &mut self,
        r: ReflectionId,
        d: DeclId,
        (vt, tc): (QualType, QualType),
        loc: Loc,
        oven: &mut Oven,
    ) -> Result<MetaOut, MetaError> {
        // A reference parameter: its referent, which the frame's register
        // names (WP-1505); a reference local: the object its block holds
        // the referent's address in, made because it is reflected
        // (WP-1515).
        let is_ref = self.ast.types.is_reference(vt);
        let vt = if is_ref {
            let t = &self.ast.types;
            t.pointee(t.canonical(vt)).map_or(vt, |p| t.canonical(p))
        } else {
            vt
        };
        let found = if !is_ref {
            oven.meta_local(d)
        } else if self.ast.decls.get(d).kind() == DeclKind::ParmVar {
            oven.meta_ref_param(d)
        } else {
            oven.meta_ref_local(d)
        };
        let Some(addr) = found else {
            let msg = format!(
                "'{}' is not a variable of static storage duration, nor one whose lifetime began \
                 within the evaluation",
                self.reflection_str(r)
            );
            // The variable, whose block ended (or whose frame returned)
            // before the reflection was extracted.
            let decl = Note::new(self.ast.decls.get(d).loc(), "declared here");
            return Err(MetaError::failed(vec![Note::new(loc, msg), decl]));
        };
        let t = &self.ast.types;
        let u = t.canonical(vt);
        if t.is_reference(tc) {
            let tt = t.pointee(tc).map_or(QualType::ERROR, |q| t.canonical(q));
            if self.qual_compatible(u, tt, u.quals().is_const()) {
                return Ok(MetaOut::Live { addr, by_ref: true });
            }
        } else if tc.without_quals() == u.without_quals() && !t.is_array(u) {
            return Ok(MetaOut::Live { addr, by_ref: false });
        }
        self.mismatch(r, tc, loc)
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
            // An element that is an array (LWG4483: `const int (*)[3]`
            // into a `reflect_constant_array` of `int[3][3]`) matches
            // bound by bound, the qualifiers on its innermost element.
            if self.qual_compatible(e, pt, true) {
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
        // [meta.reflection.extract]/10.4: a closure object converts to a
        // function pointer type through its conversion function.
        if t.is_pointer(tc)
            && let Some(c) = t.as_record(u)
            && self.ast.decls.record(self.class_def(c)).is_some_and(|x| x.lambda.is_some())
        {
            return self.extract_closure_pointer(r, self.class_def(c), tc, loc);
        }
        self.mismatch(r, tc, loc)
    }

    /// `extract<T>` of a closure object of type `c` as function pointer
    /// type `tc` (/10.4): the function its conversion function to `tc`
    /// (or to its noexcept counterpart) returns, the static invoker or a
    /// static call operator, with no state to read. A generic lambda's
    /// conversion function template is not looked at.
    fn extract_closure_pointer(
        &mut self,
        r: ReflectionId,
        c: DeclId,
        tc: QualType,
        loc: Loc,
    ) -> Result<MetaOut, MetaError> {
        let want = self.ast.types.canonical(tc.without_quals());
        let convs: Vec<DeclId> =
            self.ast.members(c).filter(|&m| self.kind_of(m) == DeclKind::CXXConversion).collect();
        for conv in convs {
            let Some(kiln_ast::lambda::ClosureThunk::Conversion { target }) =
                self.ast.closure_thunk(conv)
            else {
                continue;
            };
            let Some(ft) = self.ast.decls.get(target).ty() else { continue };
            let ft = self.unqual(ft);
            let Some(w) = self.ast.types.pointee(want).map(|q| self.ast.types.canonical(q)) else {
                continue;
            };
            if w != ft && !self.same_but_noexcept(w, ft) {
                continue;
            }
            self.define_lambda_conversion(conv, loc);
            return self.extract_function(r, target, tc, loc);
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
