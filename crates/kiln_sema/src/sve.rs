//! The `AArch64` SVE ACLE sizeless builtin types (spec/types.md §5.1):
//! `__SVInt8_t` ... `__SVBool_t`, `__SVCount_t`.
//!
//! clang declares them as implicit translation-unit typedefs on `AArch64`
//! (`Sema::Initialize`, `AArch64SVEACLETypes.def`). They are complete
//! types without a size: `sizeof`/`alignof`, arrays, fields and variables
//! of static or thread storage duration are errors (clang
//! `err_sizeof_alignof_typeof_sizeless`, `err_array_incomplete_or_sizeless_type`,
//! `err_field_incomplete_or_sizeless`, `err_sizeless_nonlocal`). kiln has
//! no SVE target feature, so — as clang without `+sve` — every use inside
//! a function body is an error (clang `checkTypeSupport`,
//! `err_sve_vector_in_non_sve_target`): only declarations can name them,
//! and codegen lowers them to scalable vectors in declarations only.

use kiln_ast::decl::{DeclFlags, DeclKind, TlsKind, VarStorage};
use kiln_ast::expr::{BinaryOp, CastKind, ValueCategory};
use kiln_ast::names::DeclName;
use kiln_ast::types::BuiltinType;
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

    /// clang `err_sve_vector_in_non_sve_target` for each SVE type a use of
    /// type `q` at `loc` needs.
    pub(crate) fn check_sve_support(&mut self, q: QualType, loc: Loc) {
        for x in self.sve_types_of(q) {
            let msg = self.diag(DiagId::sema_sve_type_without_sve, loc).arg(self.ty_arg(x));
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
    /// at namespace scope: `auto p = &f;`). clang 19 checks parameters and
    /// functions here (a local variable was diagnosed at its declaration),
    /// and not in an unevaluated operand.
    pub(crate) fn check_sve_use(&mut self, d: DeclId, loc: Loc) {
        if self.unevaluated > 0
            || !matches!(
                self.ast.decls.get(d).kind(),
                DeclKind::ParmVar | DeclKind::Function | DeclKind::CXXMethod
            )
        {
            return;
        }
        if let Some(t) = self.ast.decls.get(d).ty() {
            self.check_sve_support(t, loc);
        }
    }

    /// A variable `v` of sizeless type: an error unless it has automatic
    /// storage duration ([basic.stc]), and then one without SVE support.
    /// Either way `v` is invalid, so its uses raise no further errors
    /// (clang marks the declaration invalid after either diagnostic).
    pub(crate) fn check_sizeless_var(&mut self, v: DeclId, local: bool) {
        let n = *self.ast.decls.get(v);
        let Some(ty) = n.ty().filter(|&t| self.ast.types.is_sizeless(t)) else { return };
        let spec = self.ast.decls.var(v).map(|x| x.spec).unwrap_or_default();
        let automatic = local
            && !matches!(spec.storage(), VarStorage::Static | VarStorage::Extern)
            && spec.tls() == TlsKind::None;
        if automatic {
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
