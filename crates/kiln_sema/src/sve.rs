//! The `AArch64` SVE ACLE sizeless builtin types (spec/types.md §5.1):
//! `__SVInt8_t` ... `__SVBool_t`, `__SVCount_t`.
//!
//! clang declares them as implicit translation-unit typedefs on `AArch64`
//! (`Sema::Initialize`, `AArch64SVEACLETypes.def`). They are complete
//! types without a size: `sizeof`/`alignof`, arrays, fields and variables
//! of static or thread storage duration are errors (clang
//! `err_sizeof_alignof_typeof_sizeless`, `err_array_incomplete_or_sizeless_type`,
//! `err_field_incomplete_or_sizeless`, `err_sizeless_nonlocal`). Without
//! the `sve` target feature ([`Sema::sve`], `-march=armv8-a+sve`) every
//! use inside a function body is an error, as clang's (`checkTypeSupport`,
//! `err_sve_vector_in_non_sve_target`): only declarations can name them.
//! With it, automatic variables, parameters and the built-in operators
//! on them are valid; codegen lowers the types to scalable vectors.

use kiln_ast::decl::{DeclFlags, DeclKind, TlsKind, VarStorage};
use kiln_ast::expr::{BinaryOp, CastKind, ValueCategory};
use kiln_ast::names::DeclName;
use kiln_ast::types::{BuiltinType, VectorKind};
use kiln_ast::{DeclHead, DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::vector_ops::compound_base;
use crate::{Sema, conv};

/// clang's `getBuiltinVectorTypeInfo` for the SVE vector builtins: the
/// element type and minimum element count (`None` for `__SVCount_t`).
fn sve_info(b: BuiltinType) -> Option<(QualType, u32)> {
    use BuiltinType as B;
    Some(match b {
        B::SvInt8 => (QualType::SCHAR, 16),
        B::SvInt16 => (QualType::SHORT, 8),
        B::SvInt32 => (QualType::INT, 4),
        B::SvInt64 => (QualType::LONG, 2),
        B::SvUint8 => (QualType::UCHAR, 16),
        B::SvUint16 => (QualType::USHORT, 8),
        B::SvUint32 => (QualType::UINT, 4),
        B::SvUint64 => (QualType::ULONG, 2),
        B::SvFloat16 => (QualType::HALF, 8),
        B::SvBfloat16 => (QualType::BFLOAT16, 8),
        B::SvFloat32 => (QualType::FLOAT, 4),
        B::SvFloat64 => (QualType::DOUBLE, 2),
        B::SvBool => (QualType::BOOL, 16),
        _ => return None,
    })
}

/// clang's `ArithConvKind` for `CheckSizelessVectorOperands`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SveOpKind {
    Arithmetic,
    Bitwise,
    Comparison,
    Conditional,
}

/// The kind of element clang's `getScalableVectorType` matches a scalar
/// against: integers by signedness, `bool`, `__bf16`, other floating.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SveElemKind {
    Int(bool),
    Bool,
    BFloat,
    Float,
}

/// The element-name part of clang's SVE tuple typedefs
/// (`__clang_svint64x2_t`) for the vector builtin `b`.
fn sve_tuple_stem(b: BuiltinType) -> &'static str {
    use BuiltinType as B;
    match b {
        B::SvInt8 => "int8",
        B::SvInt16 => "int16",
        B::SvInt32 => "int32",
        B::SvInt64 => "int64",
        B::SvUint8 => "uint8",
        B::SvUint16 => "uint16",
        B::SvUint32 => "uint32",
        B::SvUint64 => "uint64",
        B::SvFloat16 => "float16",
        B::SvFloat32 => "float32",
        B::SvFloat64 => "float64",
        B::SvBfloat16 => "bfloat16",
        _ => "bool",
    }
}

impl Sema<'_, '_> {
    /// Declares the implicit typedefs naming the SVE builtins on `AArch64`.
    pub(crate) fn declare_sve_typedefs(&mut self) {
        if self.abi.triple.arch != kiln_abi::target::Arch::AArch64 {
            return;
        }
        let tu = self.ast.tu;
        for b in BuiltinType::ALL.into_iter().filter(|b| b.is_sizeless()) {
            let n = DeclName::ident(self.ast.symbols().intern(b.name()));
            let h = DeclHead::new(tu, n, Loc::INVALID).flags(DeclFlags::IMPLICIT);
            let _ = self.builder().typedef(h, DeclKind::Typedef, b.qual_type(), None);
        }
    }

    /// clang `handleArmBuiltinAliasAttr` (`arm`) / `handleBuiltinAliasAttr`
    /// (WP-810): one identifier naming an `AArch64` ACLE builtin. clang
    /// accepts the SVE and SME builtins (`SveAliasValid`); kiln knows them
    /// by their `__builtin_sve_` / `__builtin_sme_` prefix, so a misspelled
    /// one is accepted and every call of the function is codegen's
    /// "not supported yet" error. `__clang_arm_builtin_alias` is a target
    /// attribute (`TargetAnyArm`): unknown, and ignored, on x86-64.
    pub(crate) fn builtin_alias_attribute(
        &mut self,
        arm: bool,
        syntax: kiln_ast::attr::AttrSyntax,
        shown: &str,
        args: &[crate::decl::AttrArg],
        loc: Loc,
    ) -> Option<kiln_ast::AttrId> {
        let aarch64 = self.abi.triple.arch == kiln_abi::target::Arch::AArch64;
        if arm && !aarch64 {
            let msg = self.diag(DiagId::parse_unknown_attribute, loc).arg(shown);
            self.emit(msg);
            return None;
        }
        let [arg] = args else {
            let msg = self.diag(DiagId::parse_attribute_wrong_arg_count, loc).arg(shown).arg("1");
            self.emit(msg);
            return None;
        };
        let crate::decl::AttrArg::Ident(sym, _) = *arg else {
            let msg = self
                .diag(DiagId::sema_attribute_param_not_ident, loc)
                .arg(format!("'{shown}'"))
                .arg("1");
            self.emit(msg);
            return None;
        };
        let name = self.ast.symbols().as_str(sym);
        let acle = name.starts_with("__builtin_sve_") || name.starts_with("__builtin_sme_");
        if !(aarch64 && acle) {
            let msg = if arm {
                self.diag(DiagId::sema_attribute_arm_builtin_alias, loc)
            } else {
                self.diag(DiagId::sema_attribute_builtin_alias_target, loc)
                    .arg(format!("'{shown}'"))
            };
            self.emit(msg);
            return None;
        }
        let node = kiln_ast::attr::AttrNode {
            kind: kiln_ast::attr::AttrKind::BUILTIN_ALIAS,
            syntax,
            flags: 0,
            loc,
            a: sym.index(),
            b: u32::from(arm),
        };
        Some(self.ast.attrs.push(node))
    }

    /// The SVE types a use of a value of type `q` needs (clang
    /// `checkTypeSupport`): `q` itself, or a function type's parameter
    /// types and then its return type.
    fn sve_types_of(&self, q: QualType) -> Vec<QualType> {
        let t = &self.ast.types;
        if t.is_sizeless(q) {
            return vec![q];
        }
        // The written (sugared) types when the node is a function type.
        let Some(p) = t.proto_of(q.ty()).or_else(|| t.proto(q)) else { return Vec::new() };
        t.params(p)
            .iter()
            .copied()
            .chain(std::iter::once(p.ret()))
            .filter(|&x| t.is_sizeless(x))
            .collect()
    }

    /// The function whose target features a use in the current context
    /// is checked against (clang `checkTypeSupport`'s `FD`: the current
    /// lexical context when it is a function, a lambda body's call
    /// operator included).
    fn sve_context_fn(&self) -> Option<DeclId> {
        Some(self.ctx).filter(|&c| self.ast.decls.get(c).kind().is_function())
    }

    /// Whether function `f`'s features (`None`: the translation unit's)
    /// have `sve` and `sme` (clang `getFunctionFeatureMap`): the module's
    /// ([`Sema::sve`], [`Sema::target_features`]), then those of the
    /// `target("...")` attribute of `f` or a previous declaration, written
    /// or pushed by `#pragma clang attribute` (an instantiation carries its
    /// pattern's). `+sve2` and the architecture levels that include SVE
    /// imply `+sve` through the parser's feature closure.
    pub(crate) fn sve_features_of(&self, f: Option<DeclId>) -> (bool, bool) {
        let state = |fs: &[String], name: &str| {
            fs.iter().rev().find_map(|e| match e.split_at_checked(1) {
                Some((sign, n)) if n == name => Some(sign == "+"),
                _ => None,
            })
        };
        let mut sve = self.sve;
        let mut sme = state(&self.target_features, "sme").unwrap_or(false);
        let attr = f.and_then(|f| self.find_attr(f, kiln_ast::attr::AttrKind::TARGET));
        let attr = attr.map(|a| *self.ast.attrs.get(a));
        if let Some(n) = attr
            && let Some(lit) = <kiln_ast::StringLitId as kiln_ast::ids::RawId>::from_raw(n.a)
        {
            let text = String::from_utf8_lossy(self.ast.strings.bytes(lit)).into_owned();
            let module: Vec<String> = ["+fp-armv8", "+neon", "+v8a"]
                .into_iter()
                .map(str::to_owned)
                .chain(self.target_features.iter().cloned())
                .collect();
            let fs = (self.target_attr_features)(&text, &module, self.clang_compat >= 22);
            sve = state(&fs, "sve").unwrap_or(sve);
            sme = state(&fs, "sme").unwrap_or(sme);
        }
        (sve, sme)
    }

    /// clang `err_sve_vector_in_non_sve_target` for each SVE type a use of
    /// type `q` at `loc` needs, checked against the current function's
    /// features ([`Sema::sve_features_of`]); nothing with `sve`.
    pub(crate) fn check_sve_support(&mut self, q: QualType, loc: Loc) {
        self.check_sve_support_in(self.sve_context_fn(), q, loc);
    }

    /// [`Sema::check_sve_support`] against function `f`'s features: with
    /// `sme` but not `sve`, clang's `err_sve_vector_in_non_streaming_function`
    /// (kiln has no streaming functions: the `__arm_streaming` keyword
    /// attributes are not parsed).
    pub(crate) fn check_sve_support_in(&mut self, f: Option<DeclId>, q: QualType, loc: Loc) {
        let types = self.sve_types_of(q);
        if types.is_empty() {
            return;
        }
        let (sve, sme) = self.sve_features_of(f);
        if sve {
            return;
        }
        let id = if sme {
            DiagId::sema_sve_type_non_streaming
        } else {
            DiagId::sema_sve_type_without_sve
        };
        for x in types {
            let msg = self.diag(id, loc).arg(self.ty_arg(x));
            self.emit(msg.allow_duplicate());
        }
    }

    /// The operands `ops` of a built-in binary operator at `loc` (clang
    /// `CreateBuiltinBinOp`'s `checkTypeSupport` of each operand type at
    /// the operator, with no declaration): diagnosed only inside a
    /// function and not in an unevaluated operand; a dependent operand
    /// is checked as it is instantiated.
    pub(crate) fn check_sve_operands(&mut self, ops: [ExprId; 2], loc: Loc) {
        if self.unevaluated > 0 || !self.ast.decls.get(self.ctx).kind().is_function() {
            return;
        }
        for e in ops {
            let t = self.ety(e);
            if !self.ast.types.is_dependent(t) {
                self.check_sve_support(t, loc);
            }
        }
    }

    /// A reference to `d`: its type must be supported by the target (also
    /// at namespace scope: `auto p = &f;`). clang 19 checks parameters,
    /// variables and functions here (clang `DiagnoseUseOfDecl`), and not
    /// in an unevaluated operand. A local variable without support was
    /// diagnosed at its declaration and is invalid, so only one named in
    /// a lambda whose call operator lacks the feature is reported here
    /// (the capture's every use).
    pub(crate) fn check_sve_use(&mut self, d: DeclId, loc: Loc) {
        if self.unevaluated > 0
            || !matches!(
                self.ast.decls.get(d).kind(),
                DeclKind::ParmVar | DeclKind::Var | DeclKind::Function | DeclKind::CXXMethod
            )
        {
            return;
        }
        if let Some(t) = self.ast.decls.get(d).ty() {
            // clang's `FD`: the current function, else the referenced one.
            let f = self
                .sve_context_fn()
                .or_else(|| Some(d).filter(|&d| self.ast.decls.get(d).kind().is_function()));
            self.check_sve_support_in(f, t, loc);
        }
    }

    /// A variable `v` of sizeless type: an error unless it has automatic
    /// storage duration ([basic.stc]), and then one without SVE support.
    /// After either error `v` is invalid, so its uses raise no further
    /// errors (clang marks the declaration invalid after either
    /// diagnostic); an automatic variable with the `sve` feature is valid.
    pub(crate) fn check_sizeless_var(&mut self, v: DeclId, local: bool) {
        let n = *self.ast.decls.get(v);
        let Some(ty) = n.ty().filter(|&t| self.ast.types.is_sizeless(t)) else { return };
        let spec = self.ast.decls.var(v).map(|x| x.spec).unwrap_or_default();
        let automatic = local
            && !matches!(spec.storage(), VarStorage::Static | VarStorage::Extern)
            && spec.tls() == TlsKind::None;
        if automatic {
            if self.sve_features_of(self.sve_context_fn()).0 {
                return;
            }
            self.check_sve_support(ty, n.loc());
        } else {
            let msg = self.diag(DiagId::sema_nonlocal_sizeless, n.loc()).arg(self.ty_arg(ty));
            self.emit(msg);
        }
        self.ast.decls.set_flags(v, DeclFlags::INVALID);
    }

    /// The element type and count of the SVE vector type `q` (clang
    /// `isSveVLSBuiltinType`: every sizeless type but `__SVCount_t`).
    pub(crate) fn sve_parts(&self, q: QualType) -> Option<(QualType, u32)> {
        self.ast.types.builtin(q).and_then(sve_info)
    }

    /// clang's `isSveVLSBuiltinType`.
    pub(crate) fn is_sve_vector(&self, q: QualType) -> bool {
        self.sve_parts(q).is_some()
    }

    /// The SVE vector type of `count` elements of type `elem` (clang
    /// `getScalableVectorType`).
    fn sve_vector_of(elem: QualType, count: u32) -> QualType {
        BuiltinType::ALL
            .into_iter()
            .find(|&b| sve_info(b) == Some((elem, count)))
            .map_or(QualType::ERROR, BuiltinType::qual_type)
    }

    /// clang's `GetSignedSizelessVectorType`: the signed integer SVE
    /// vector with `v`'s element size and count.
    fn signed_sve_vector(&self, v: QualType) -> QualType {
        let Some((elem, count)) = self.sve_parts(v) else { return v };
        let bits = self.scalar_bits(elem).unwrap_or(0);
        [QualType::SCHAR, QualType::SHORT, QualType::INT, QualType::LONG]
            .into_iter()
            .find(|&t| self.scalar_bits(t) == Some(bits))
            .map_or(QualType::ERROR, |e| Self::sve_vector_of(e, count))
    }

    /// A built-in binary or compound-assignment operator with an SVE
    /// vector operand (not `=`, `,`, `.*`, `->*`, `&&`, `||`), as
    /// clang's `CheckSizelessVectorOperands`,
    /// `CheckSizelessVectorCompareOperands` and `checkSizelessVectorShift`
    /// decide it (spec/sema.md §5.8).
    pub(crate) fn sve_binary(&mut self, op: BinaryOp, l: ExprId, r: ExprId, loc: Loc) -> ExprId {
        let comp = op.is_compound_assignment();
        let base = if comp { compound_base(op) } else { op };
        let (lt0, rt0) = (self.ety(l), self.ety(r));
        let (mut l2, mut r2) = (l, r);
        let res = match base {
            BinaryOp::Shl | BinaryOp::Shr => self.sve_shift(&mut l2, &mut r2, loc, comp),
            BinaryOp::Mul | BinaryOp::Div | BinaryOp::Add | BinaryOp::Sub => {
                self.check_sve_vector_operands(&mut l2, &mut r2, loc, comp, SveOpKind::Arithmetic)
            }
            BinaryOp::Rem | BinaryOp::And | BinaryOp::Xor | BinaryOp::Or => {
                if !self.has_integer_repr(lt0) || !self.has_integer_repr(rt0) {
                    return self.invalid_operands(l, r, loc);
                }
                let kind =
                    if base == BinaryOp::Rem { SveOpKind::Arithmetic } else { SveOpKind::Bitwise };
                self.check_sve_vector_operands(&mut l2, &mut r2, loc, comp, kind)
            }
            BinaryOp::LT
            | BinaryOp::GT
            | BinaryOp::LE
            | BinaryOp::GE
            | BinaryOp::EQ
            | BinaryOp::NE => {
                let v = self.check_sve_vector_operands(
                    &mut l2,
                    &mut r2,
                    loc,
                    false,
                    SveOpKind::Comparison,
                );
                v.map(|v| {
                    self.diagnose_self_comparison(base, l2, r2, loc);
                    let both_bool = [l2, r2]
                        .map(|e| self.ety(e))
                        .iter()
                        .all(|&t| self.ast.types.builtin(t) == Some(BuiltinType::SvBool));
                    if both_bool { v } else { self.signed_sve_vector(v) }
                })
            }
            BinaryOp::Cmp => {
                self.error(DiagId::sema_three_way_vector_comparison, loc);
                None
            }
            _ => return self.invalid_operands(l, r, loc),
        };
        let Some(ty) = res else {
            return self.recover_err(&[l, r], loc);
        };
        if !comp {
            return self.builder().binary(op, l2, r2, ty, loc);
        }
        if !self.check_modifiable(l, loc) {
            return self.recover_err(&[l, r], loc);
        }
        let lhs = conv::strip_cv(self, lt0);
        if !self.ast.types.same_unqualified(ty, lhs) {
            let (x, y) = self.ty_arg_pair(lt0, ty);
            let msg = self.diag(DiagId::sema_assign_incompatible, loc).arg(x).arg(y);
            self.emit(msg);
            return self.recover_err(&[l, r], loc);
        }
        self.builder().compound_assign(op, l, r2, lt0, ty, ty, loc)
    }

    /// True if `q` is an SVE vector type whose element is integral (clang
    /// `isValidSizelessVectorForConditionalCondition`, `__SVBool_t`
    /// included): `q ? t : f` selects element-wise.
    pub(crate) fn is_sve_condition(&self, q: QualType) -> bool {
        self.sve_parts(q).is_some_and(|(e, _)| self.ast.types.is_integral(e))
    }

    /// `c ? t : f` with an SVE vector condition `c` (clang
    /// `CXXCheckConditionalOperands` + `CheckSizelessVectorConditionalTypes`;
    /// spec/types.md §5.1). A `void` arm is an error; two SVE arms must
    /// have the same type, which is the result; one SVE arm goes through
    /// `CheckSizelessVectorOperands`; two scalar arms get their common
    /// type, splatted to the SVE vector of the condition's element count.
    /// The result must have the condition's element count and size.
    pub(crate) fn sve_conditional(&mut self, c: ExprId, t: ExprId, f: ExprId, loc: Loc) -> ExprId {
        let c2 = conv::decay(self, c);
        if self.expr_is_type_dependent(t) || self.expr_is_type_dependent(f) {
            return self.builder().dependent_conditional(c2, t, f, loc);
        }
        for v in [t, f] {
            if self.ast.types.is_void(self.ety(v)) {
                let at = self.begin_loc(v);
                let throw = if self.is_throw_expr(v) { "1" } else { "0" };
                let msg = self.diag(DiagId::sema_conditional_vector_has_void, at).arg(throw);
                self.emit(msg);
                return self.recover_err(&[c, t, f], loc);
            }
        }
        let ct = conv::strip_cv(self, self.ety(c2));
        let Some((celem, ccount)) = self.sve_parts(ct) else {
            return self.recover_err(&[c, t, f], loc);
        };
        let mut t2 = conv::decay(self, t);
        let mut f2 = conv::decay(self, f);
        let lt = conv::strip_cv(self, self.ety(t2));
        let rt = conv::strip_cv(self, self.ety(f2));
        let res = match (self.is_sve_vector(lt), self.is_sve_vector(rt)) {
            (true, true) => {
                if !self.ast.types.same_type(lt, rt) {
                    self.vector_diag2(DiagId::sema_conditional_vector_mismatched, lt, rt, loc);
                    return self.recover_err(&[c, t, f], loc);
                }
                Some(lt)
            }
            (true, false) | (false, true) => {
                self.check_sve_vector_operands(&mut t2, &mut f2, loc, false, SveOpKind::Conditional)
            }
            (false, false) => self.sve_cond_scalar_arms(ct, ccount, &mut t2, &mut f2, loc),
        };
        let Some(res) = res else {
            return self.recover_err(&[c, t, f], loc);
        };
        let Some((relem, rcount)) = self.sve_parts(res) else {
            return self.recover_err(&[c, t, f], loc);
        };
        if rcount != ccount {
            self.vector_diag2(DiagId::sema_conditional_vector_size, ct, res, loc);
            return self.recover_err(&[c, t, f], loc);
        }
        if self.scalar_bits(relem) != self.scalar_bits(celem) {
            self.vector_diag2(DiagId::sema_conditional_vector_element_size, ct, res, loc);
            return self.recover_err(&[c, t, f], loc);
        }
        self.builder().conditional(c2, t2, f2, res, loc)
    }

    /// `CheckSizelessVectorConditionalTypes` with two scalar arms: their
    /// common type, splatted to the SVE vector of `count` such elements.
    /// `None` after a diagnostic.
    fn sve_cond_scalar_arms(
        &mut self,
        ct: QualType,
        count: u32,
        t2: &mut ExprId,
        f2: &mut ExprId,
        loc: Loc,
    ) -> Option<QualType> {
        let lt = conv::strip_cv(self, self.ety(*t2));
        let rt = conv::strip_cv(self, self.ety(*f2));
        let elem = if self.ast.types.same_type(lt, rt) {
            lt
        } else {
            match conv::usual_arithmetic_conversions_kind(
                self,
                *t2,
                *f2,
                conv::ArithConvKind::Conditional,
            ) {
                Ok((a, b, ty)) => {
                    self.report_arith_warning(loc);
                    (*t2, *f2) = (a, b);
                    ty
                }
                Err(err) => {
                    self.emit_conv_error(err, loc);
                    return None;
                }
            }
        };
        if self.ast.types.as_enum(elem).is_some() {
            let msg =
                self.diag(DiagId::sema_conditional_vector_operand_type, loc).arg(self.ty_arg(elem));
            self.emit(msg);
            return None;
        }
        match self.sve_vector_for_scalar(elem, count) {
            Ok(vt) => {
                let pr = ValueCategory::PRValue;
                *t2 = conv::raw_cast(self, CastKind::VectorSplat, *t2, vt, pr, None);
                *f2 = conv::raw_cast(self, CastKind::VectorSplat, *f2, vt, pr, None);
                Some(vt)
            }
            // clang finds the tuple type, whose element count is never the
            // condition's; kiln has no SVE tuple types and names it.
            Err(Some(tuple)) => {
                let msg = self
                    .diag(DiagId::sema_conditional_vector_size, loc)
                    .arg(self.ty_arg(ct))
                    .arg(format!("'{tuple}'"));
                self.emit(msg);
                None
            }
            Err(None) => {
                let msg = self
                    .diag(DiagId::sema_conditional_vector_scalar_type_unsupported, loc)
                    .arg(self.ty_arg(elem))
                    .arg(self.ty_arg(ct));
                self.emit(msg);
                None
            }
        }
    }

    /// The `getScalableVectorType` kind of scalar or element type `q`.
    fn sve_elem_kind(&self, q: QualType) -> Option<SveElemKind> {
        let b = self.ast.types.builtin(q)?;
        Some(if b == BuiltinType::Bool {
            SveElemKind::Bool
        } else if b.is_integral() {
            SveElemKind::Int(conv::rank::builtin_signed(self, b))
        } else if b == BuiltinType::BFloat16 {
            SveElemKind::BFloat
        } else if b.is_floating() {
            SveElemKind::Float
        } else {
            return None;
        })
    }

    /// clang `getScalableVectorType(elem, count)`: the SVE vector of
    /// `count` elements like `elem` (same kind and size), else the name
    /// of the SVE tuple type of that many (`Err(Some)`), else `Err(None)`.
    fn sve_vector_for_scalar(
        &self,
        elem: QualType,
        count: u32,
    ) -> Result<QualType, Option<String>> {
        let kind = self.sve_elem_kind(elem).ok_or(None)?;
        let bits = self.scalar_bits(elem).ok_or(None)?;
        for b in BuiltinType::ALL {
            let Some((e, n)) = sve_info(b) else { continue };
            if self.sve_elem_kind(e) != Some(kind) || self.scalar_bits(e) != Some(bits) {
                continue;
            }
            let nf = if count.is_multiple_of(n) { count / n } else { 0 };
            let tuple =
                if kind == SveElemKind::Bool { matches!(nf, 2 | 4) } else { (2..=4).contains(&nf) };
            if nf == 1 {
                return Ok(b.qual_type());
            }
            if tuple {
                return Err(Some(format!("__clang_sv{}x{nf}_t", sve_tuple_stem(b))));
            }
        }
        Err(None)
    }

    /// clang's `CheckSizelessVectorOperands`: converts the operands in
    /// place (a scalar is splatted when that cannot truncate) and returns
    /// the result type, or `None` after a diagnostic.
    fn check_sve_vector_operands(
        &mut self,
        l: &mut ExprId,
        r: &mut ExprId,
        loc: Loc,
        comp: bool,
        kind: SveOpKind,
    ) -> Option<QualType> {
        if !comp {
            *l = conv::decay(self, *l);
        }
        *r = conv::decay(self, *r);
        let lt = conv::strip_cv(self, self.ety(*l));
        let rt = conv::strip_cv(self, self.ety(*r));
        let is_bool = |s: &Self, t: QualType| s.ast.types.builtin(t) == Some(BuiltinType::SvBool);
        if kind == SveOpKind::Arithmetic && (is_bool(self, lt) || is_bool(self, rt)) {
            self.vector_diag2(DiagId::sema_invalid_operands, lt, rt, loc);
            return None;
        }
        if self.ast.types.same_type(lt, rt) {
            return Some(lt);
        }
        let (lv, rv) = (self.sve_parts(lt), self.sve_parts(rt));
        if lv.is_some() && rv.is_none() {
            let vec = *l;
            if self.gcc_vector_convert_and_splat(r, vec) {
                return Some(lt);
            }
        }
        if rv.is_some() && lv.is_none() {
            let vec = *r;
            if comp || self.gcc_vector_convert_and_splat(l, vec) {
                return Some(rt);
            }
        }
        if (lv.is_none() && !self.is_scalar_real(lt)) || (rv.is_none() && !self.is_scalar_real(rt))
        {
            self.vector_diag2(DiagId::sema_vector_not_convertible_non_scalar, lt, rt, loc);
            return None;
        }
        if let (Some(a), Some(b)) = (lv, rv)
            && a.1 != b.1
        {
            self.vector_diag2(DiagId::sema_vector_lengths_not_equal, lt, rt, loc);
            return None;
        }
        let (scalar, vector) = if lv.is_some() { (rt, lt) } else { (lt, rt) };
        let sel = if lv.is_some() && rv.is_some() { "1" } else { "0" };
        let (x, y) = self.ty_arg_pair(scalar, vector);
        let msg = self.diag(DiagId::sema_vector_implicit_truncation, loc).arg(sel).arg(x).arg(y);
        self.emit(msg);
        None
    }

    /// clang's `checkSizelessVectorShift`.
    fn sve_shift(
        &mut self,
        l: &mut ExprId,
        r: &mut ExprId,
        loc: Loc,
        comp: bool,
    ) -> Option<QualType> {
        if !comp {
            *l = conv::usual_unary_conversions(self, *l);
        }
        *r = conv::usual_unary_conversions(self, *r);
        let lt = conv::strip_cv(self, self.ety(*l));
        let rt = conv::strip_cv(self, self.ety(*r));
        let (lv, rv) = (self.sve_parts(lt), self.sve_parts(rt));
        let is_bool = |s: &Self, t: QualType| s.ast.types.builtin(t) == Some(BuiltinType::SvBool);
        if is_bool(self, lt) || is_bool(self, rt) {
            self.vector_diag2(DiagId::sema_invalid_operands, lt, rt, loc);
            return None;
        }
        let lelem = lv.map_or(lt, |p| p.0);
        let relem = rv.map_or(rt, |p| p.0);
        for (elem, t) in [(lelem, lt), (relem, rt)] {
            if !self.ast.types.is_integral(elem) {
                let msg = self.diag(DiagId::sema_expect_int, loc).arg(self.ty_arg(t));
                self.emit(msg);
                return None;
            }
        }
        let splat = |s: &mut Self, e: ExprId, from: QualType, elem: QualType, count: u32| {
            let x = if s.ast.types.same_unqualified(from, elem) {
                e
            } else {
                conv::cast_to(s, CastKind::IntegralCast, e, elem, None)
            };
            let vt = Self::sve_vector_of(elem, count);
            conv::raw_cast(s, CastKind::VectorSplat, x, vt, ValueCategory::PRValue, None)
        };
        match (lv, rv) {
            (Some(a), Some(b)) => {
                if a.1 != b.1 {
                    self.vector_diag2(DiagId::sema_invalid_operands, lt, rt, loc);
                    return None;
                }
                Some(lt)
            }
            (None, Some(b)) => {
                if !comp {
                    *l = splat(self, *l, lelem, relem, b.1);
                }
                Some(rt)
            }
            (Some(a), None) => {
                *r = splat(self, *r, relem, lelem, a.1);
                Some(lt)
            }
            (None, None) => None,
        }
    }
}

/// The attribute name as clang's diagnostics quote it.
const SVE_BITS: &str = "'arm_sve_vector_bits'";

impl Sema<'_, '_> {
    /// GNU `arm_sve_vector_bits(N)` as written (clang
    /// `HandleArmSveVectorBitsTypeAttr`, first half): the target must have
    /// `sve` and `-msve-vector-bits=` must fix the vector length before
    /// the argument is looked at; then exactly one expression argument,
    /// kept for [`Self::build_sve_fixed_type`].
    pub(crate) fn sve_bits_attribute(
        &mut self,
        syntax: kiln_ast::attr::AttrSyntax,
        raw: &str,
        args: &[crate::decl::AttrArg],
        loc: Loc,
    ) -> Option<kiln_ast::AttrId> {
        if !self.sve {
            let msg = self
                .diag(DiagId::sema_attribute_unsupported_target, loc)
                .arg(SVE_BITS)
                .arg("'sve'");
            self.emit(msg);
            return None;
        }
        if self.sve_vector_bits.is_none() {
            let msg = self.diag(DiagId::sema_sve_bits_unsupported, loc).arg(SVE_BITS);
            self.emit(msg);
            return None;
        }
        let kind = kiln_ast::attr::AttrKind::ARM_SVE_VECTOR_BITS;
        self.vector_attribute(kind, syntax, raw, args, loc)
    }

    /// `ty` with `arm_sve_vector_bits(bits)` applied (clang
    /// `HandleArmSveVectorBitsTypeAttr`, second half; spec/sema.md "SVE"):
    /// `bits` an integer constant equal to `-msve-vector-bits`, `ty` a
    /// sizeless SVE vector or predicate type (not `svcount_t`), and the
    /// declaration a typedef (an error that keeps the fixed type). A data
    /// vector becomes `bits / element bits` elements of its element type,
    /// `svbool_t` `bits / 64` bytes. `ty` unchanged after any other
    /// diagnostic.
    pub(crate) fn build_sve_fixed_type(
        &mut self,
        ty: QualType,
        bits: ExprId,
        typedef: bool,
        loc: Loc,
    ) -> QualType {
        let want = self.sve_vector_bits.unwrap_or(0);
        if self.expr_is_value_dependent(bits) {
            let msg =
                self.diag(DiagId::sema_attribute_requires_integer_constant, loc).arg(SVE_BITS);
            self.emit(msg);
            return ty;
        }
        let Some(n) = self.vector_size_value(bits, SVE_BITS, loc) else { return ty };
        if n != u128::from(want) {
            // clang reads the value as a 32-bit unsigned.
            let msg = self.diag(DiagId::sema_sve_bad_vector_size, loc).arg(n as u32).arg(want);
            self.emit(msg);
            return ty;
        }
        let Some((elem, _)) = self.sve_parts(ty) else {
            let msg =
                self.diag(DiagId::sema_sve_invalid_type, loc).arg(SVE_BITS).arg(self.ty_arg(ty));
            self.emit(msg);
            return ty;
        };
        // clang applies the type attribute and then rejects the
        // declaration attribute: the declared type is still the fixed one.
        if !typedef {
            let msg = self
                .diag(DiagId::sema_attribute_wrong_decl_type_error, loc)
                .arg(SVE_BITS)
                .arg("typedefs");
            self.emit(msg);
        }
        let (elem, count, kind) = if elem == QualType::BOOL {
            (QualType::UCHAR, want / 64, VectorKind::SveFixedPredicate)
        } else {
            let ebits =
                crate::target::size_align(&self.ast, elem, self.abi()).map_or(8, |s| s.0 * 8);
            (elem, want / u32::try_from(ebits).unwrap_or(8), VectorKind::SveFixedData)
        };
        let v = self.ast.types.vector(elem, count, kind);
        self.ast.types.qualify(v, kiln_ast::types::Quals::from_bits(ty.quals_bits()))
    }
}
