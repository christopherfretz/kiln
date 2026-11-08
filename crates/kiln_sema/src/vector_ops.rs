//! Operations on vector values (spec/types.md §1 `Vector`, spec/sema.md
//! §5.8): the built-in unary and binary operators, compound assignment,
//! subscripting, and the implicit vector conversions (splat, lax
//! bitcast), following clang 19's `CheckVectorOperands`,
//! `checkVectorShift`, `CheckVectorCompareOperands`,
//! `CheckVectorLogicalOperands`, `GetSignedVectorType` and
//! `IsVectorConversion`. clang is the oracle for every rule here; the
//! rules are GNU/clang extensions, not ISO C++.

use kiln_ast::consts::{FloatKind, Scalar};
use kiln_ast::expr::ValueCategory;
use kiln_ast::expr::{BinaryOp, CastKind, ExprFlags, UnaryOp};
use kiln_ast::types::{BuiltinType, TypeKind, TypeView, VectorKind};
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::conv;

/// `-flax-vector-conversions=` (clang's `LaxVectorConversionKind`): which
/// same-size vector bitcasts are implicit conversions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LaxVectorConversions {
    /// `none` (`-fno-lax-vector-conversions`): none.
    None,
    /// `integer` (a bare `-flax-vector-conversions`): only between
    /// integer vectors (and integer scalars).
    Integer,
    /// `all` (clang's default): any same-size vectors.
    #[default]
    All,
}

impl LaxVectorConversions {
    /// The kind a `-flax-vector-conversions=` value names.
    #[must_use]
    pub fn from_arg(v: &str) -> Option<Self> {
        match v {
            "none" => Some(Self::None),
            "integer" => Some(Self::Integer),
            "all" => Some(Self::All),
            _ => None,
        }
    }
}

/// A vector type's parts: canonical element, element count, kind.
#[derive(Clone, Copy, Debug)]
pub(crate) struct VecParts {
    pub elem: QualType,
    pub count: u32,
    pub kind: VectorKind,
}

impl VecParts {
    pub(crate) fn is_ext(self) -> bool {
        self.kind == VectorKind::Ext
    }
}

impl Sema<'_, '_> {
    /// The parts of vector type `q`, or `None` if `q` is not a vector.
    pub(crate) fn vector_parts(&self, q: QualType) -> Option<VecParts> {
        let c = self.ast.types.canonical(q).ty();
        match self.ast.types.view_if(c, TypeKind::Vector) {
            Some(TypeView::Vector { elem, count, kind }) => {
                Some(VecParts { elem: self.ast.types.canonical(elem).without_quals(), count, kind })
            }
            _ => None,
        }
    }

    /// True if `q` is a (non-dependent) vector type.
    pub(crate) fn is_vector_type(&self, q: QualType) -> bool {
        self.vector_parts(q).is_some()
    }

    /// clang's `isExtVectorBoolType`.
    fn is_ext_bool_vector(&self, q: QualType) -> bool {
        self.vector_parts(q).is_some_and(|p| p.is_ext() && p.elem == QualType::BOOL)
    }

    /// The size in bits of a scalar type (`None` if incomplete).
    pub(crate) fn scalar_bits(&self, q: QualType) -> Option<u64> {
        crate::target::size_align(&self.ast, q, self.abi()).map(|(s, _)| s * 8)
    }

    /// clang's `breakDownVectorType` + `areVectorTypesSameSize`: the data
    /// size of a vector (count × element size) or of a real scalar.
    pub(crate) fn lax_size(&self, q: QualType) -> Option<u64> {
        if let Some(p) = self.vector_parts(q) {
            return self.scalar_bits(p.elem).map(|b| b * u64::from(p.count));
        }
        if self.ast.types.is_arithmetic(q) || conv::is_unscoped_enum(self, q) {
            return self.scalar_bits(q);
        }
        None
    }

    /// clang's `areCompatibleVectorTypes`: same element type and count; a
    /// fixed-length SVE vector only with itself.
    pub(crate) fn compatible_vectors(&self, a: QualType, b: QualType) -> bool {
        match (self.vector_parts(a), self.vector_parts(b)) {
            (Some(x), Some(y)) if x.kind.is_sve_fixed() || y.kind.is_sve_fixed() => {
                x.kind == y.kind && x.elem == y.elem && x.count == y.count
            }
            (Some(x), Some(y)) => x.elem == y.elem && x.count == y.count,
            _ => false,
        }
    }

    /// clang's `getSveEltType`: a sizeless SVE vector's element type,
    /// `unsigned char` for `svbool_t`.
    fn sve_elt(&self, q: QualType) -> Option<QualType> {
        let (e, _) = self.sve_parts(q)?;
        Some(if e == QualType::BOOL { QualType::UCHAR } else { e })
    }

    /// clang's `getSVETypeSize`: `-msve-vector-bits` (0 without it), an
    /// eighth of that for `svbool_t`.
    fn sve_type_bits(&self, q: QualType) -> u64 {
        let bits = u64::from(self.sve_vector_bits.unwrap_or(0));
        if self.ast.types.builtin(q) == Some(BuiltinType::SvBool) { bits / 8 } else { bits }
    }

    /// clang's `areCompatibleSveTypes`: a sizeless SVE vector and the
    /// fixed-length vector made from it, or a GNU vector of its element
    /// type and `-msve-vector-bits` size.
    pub(crate) fn sve_compatible(&self, a: QualType, b: QualType) -> bool {
        let one = |x: QualType, y: QualType| {
            let (Some((elem, _)), Some(v)) = (self.sve_parts(x), self.vector_parts(y)) else {
                return false;
            };
            match v.kind {
                VectorKind::SveFixedPredicate => {
                    self.ast.types.builtin(x) == Some(BuiltinType::SvBool)
                }
                VectorKind::SveFixedData => self.sve_elt(x) == Some(v.elem),
                VectorKind::Generic => {
                    self.lax_size(y) == Some(self.sve_type_bits(x)) && v.elem == elem
                }
                _ => false,
            }
        };
        one(a, b) || one(b, a)
    }

    /// clang's `areLaxCompatibleSveTypes`: a sizeless SVE vector and a
    /// fixed-length SVE data vector or a GNU vector of the
    /// `-msve-vector-bits` size, as `-flax-vector-conversions` allows (not
    /// `svbool_t` with a data vector).
    pub(crate) fn sve_lax_compatible(&self, a: QualType, b: QualType) -> bool {
        let one = |x: QualType, y: QualType| {
            let (Some(xe), Some(v)) = (self.sve_elt(x), self.vector_parts(y)) else {
                return false;
            };
            if !matches!(v.kind, VectorKind::SveFixedData | VectorKind::Generic) {
                return false;
            }
            let bool_x = self.ast.types.builtin(x) == Some(BuiltinType::SvBool);
            if bool_x && v.kind == VectorKind::SveFixedData {
                return false;
            }
            if v.kind == VectorKind::Generic && self.lax_size(y) != Some(self.sve_type_bits(x)) {
                return false;
            }
            match self.lax_vector_conversions {
                LaxVectorConversions::All => true,
                LaxVectorConversions::Integer => {
                    self.ast.types.is_integral(v.elem) && self.ast.types.is_integral(xe)
                }
                LaxVectorConversions::None => false,
            }
        };
        one(a, b) || one(b, a)
    }

    /// clang's `isLaxVectorConversion`: same data size, never between a
    /// scalar and an ext vector; under `-flax-vector-conversions=integer`
    /// both sides integer (vectors), under `none` never.
    pub(crate) fn lax_vector_conversion(&self, from: QualType, to: QualType) -> bool {
        let (fv, tv) = (self.vector_parts(from), self.vector_parts(to));
        if fv.is_none() && tv.is_none() {
            return false;
        }
        match self.lax_vector_conversions {
            LaxVectorConversions::None => return false,
            LaxVectorConversions::Integer => {
                let integral = |q: QualType, v: Option<VecParts>| {
                    self.is_integral_or_enum(v.map_or(q, |p| p.elem))
                };
                if !integral(from, fv) || !integral(to, tv) {
                    return false;
                }
            }
            LaxVectorConversions::All => {}
        }
        if (fv.is_none() && tv.is_some_and(VecParts::is_ext))
            || (tv.is_none() && fv.is_some_and(VecParts::is_ext))
        {
            return false;
        }
        match (self.lax_size(from), self.lax_size(to)) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    /// clang's `PrepareScalarCast` between arithmetic types.
    fn scalar_cast_kind(&self, from: QualType, to: QualType) -> CastKind {
        let t = &self.ast.types;
        let to_bool = t.builtin(to) == Some(BuiltinType::Bool);
        if t.is_floating(from) {
            if to_bool {
                CastKind::FloatingToBoolean
            } else if t.is_floating(to) {
                CastKind::FloatingCast
            } else {
                CastKind::FloatingToIntegral
            }
        } else if to_bool {
            CastKind::IntegralToBoolean
        } else if t.is_floating(to) {
            CastKind::IntegralToFloating
        } else {
            CastKind::IntegralCast
        }
    }

    /// clang's `prepareVectorSplat`: the scalar prvalue `e` converted to
    /// the element type of `vec` (`true` splats to all-ones in an ext
    /// vector: `BooleanToSignedIntegral`).
    pub(crate) fn prepare_vector_splat(&mut self, vec: QualType, e: ExprId) -> ExprId {
        let Some(p) = self.vector_parts(vec) else { return e };
        let from = conv::strip_cv(self, self.ety(e));
        if self.ast.types.same_unqualified(from, p.elem) {
            return e;
        }
        if p.is_ext() && self.ast.types.builtin(from) == Some(BuiltinType::Bool) {
            if self.ast.types.is_floating(p.elem) {
                let i =
                    conv::cast_to(self, CastKind::BooleanToSignedIntegral, e, QualType::INT, None);
                return conv::cast_to(self, CastKind::IntegralToFloating, i, p.elem, None);
            }
            return conv::cast_to(self, CastKind::BooleanToSignedIntegral, e, p.elem, None);
        }
        let ck = self.scalar_cast_kind(from, p.elem);
        conv::cast_to(self, ck, e, p.elem, None)
    }

    /// clang's `GetSignedVectorType`: the signed integer vector with the
    /// element size of `v`'s (a `bool` ext vector stays itself).
    pub(crate) fn signed_vector_type(&mut self, v: QualType) -> QualType {
        let Some(p) = self.vector_parts(v) else { return v };
        if p.is_ext() && p.elem == QualType::BOOL {
            return self.ast.types.canonical(v).without_quals();
        }
        let bits = self.scalar_bits(p.elem).unwrap_or(0);
        let elem = if p.is_ext() {
            [
                QualType::CHAR,
                QualType::SHORT,
                QualType::INT,
                QualType::INT128,
                QualType::LONG,
                QualType::LONGLONG,
            ]
        } else {
            [
                QualType::INT128,
                QualType::LONGLONG,
                QualType::LONG,
                QualType::INT,
                QualType::SHORT,
                QualType::CHAR,
            ]
        }
        .into_iter()
        .find(|t| self.scalar_bits(*t) == Some(bits))
        .unwrap_or(QualType::INT);
        let kind = if p.is_ext() { VectorKind::Ext } else { VectorKind::Generic };
        self.ast.types.vector(elem, p.count, kind)
    }

    /// clang's `hasIntegerRepresentation`: an integer or unscoped
    /// enumeration type, or a vector (GNU or SVE) of integers.
    pub(crate) fn has_integer_repr(&self, q: QualType) -> bool {
        let q = self.vector_parts(q).map_or(q, |p| p.elem);
        let q = self.sve_parts(q).map_or(q, |(e, _)| e);
        self.ast.types.is_integral(q) || conv::is_unscoped_enum(self, q)
    }

    /// The two type arguments of a diagnostic: clang prints the aka and
    /// vector details only for the first occurrence of a type.
    pub(crate) fn ty_arg_pair(&self, a: QualType, b: QualType) -> (String, String) {
        let first = self.ty_arg(a);
        let second = if a == b { format!("'{}'", self.ty_str(b)) } else { self.ty_arg(b) };
        (first, second)
    }

    pub(crate) fn vector_diag2(&mut self, id: DiagId, a: QualType, b: QualType, loc: Loc) {
        let (x, y) = self.ty_arg_pair(a, b);
        let msg = self.diag(id, loc).arg(x).arg(y);
        self.emit(msg);
    }

    /// `v[i]` on a vector `v` (clang `CreateBuiltinArraySubscriptExpr`):
    /// a prvalue `v` is materialized (CWG1213), the result is the element
    /// type with `v`'s qualifiers and value category. An SVE vector
    /// (`isSveVLSBuiltinType`) is subscripted the same way, its element
    /// `getSveEltType`; `svbool_t` cannot be (`err_subscript_svbool_t`,
    /// before the index is checked).
    pub(crate) fn vector_subscript(&mut self, base: ExprId, idx: ExprId, loc: Loc) -> ExprId {
        let sve = self.sve_parts(self.ety(base)).map(|(e, _)| e);
        if sve == Some(QualType::BOOL) {
            self.error(DiagId::sema_subscript_svbool, loc);
            return self.recover_err(&[base, idx], loc);
        }
        let mut b = base;
        if self.ast.exprs.get(b).value_category() == ValueCategory::PRValue {
            let t = self.ety(b);
            b = self.materialize_temporary(b, t, false);
        }
        let i = conv::decay(self, idx);
        let it = self.ety(i);
        if !self.ast.types.is_integral(it) && !conv::is_unscoped_enum(self, it) {
            self.error(DiagId::sema_subscript_not_integer, loc);
            return self.recover_err(&[b, i], loc);
        }
        let bt = self.ety(b);
        let top = self.ast.types.desugar_top(bt);
        let elem = match (self.ast.types.view(top.ty()), sve) {
            (TypeView::Vector { elem, .. }, _) => elem,
            (_, Some(e)) => e,
            _ => return self.recover_err(&[b, i], loc),
        };
        let quals = self.ast.types.effective_quals(bt);
        let ty = QualType::new(elem.ty(), elem.quals_bits() | quals.bits());
        self.builder().vector_subscript(b, i, ty, loc)
    }

    /// A built-in unary operator on a vector operand; `None` when `op`
    /// does not apply to vectors here (the caller diagnoses).
    pub(crate) fn vector_unary(&mut self, op: UnaryOp, e: ExprId, loc: Loc) -> Option<ExprId> {
        let t0 = self.ety(e);
        let p = self.vector_parts(t0)?;
        match op {
            UnaryOp::Plus | UnaryOp::Minus => {
                let x = conv::decay(self, e);
                let ty = self.ety(x);
                let flags =
                    if op == UnaryOp::Plus { ExprFlags::CANNOT_OVERFLOW } else { ExprFlags::NONE };
                Some(self.builder().unary(op, x, ty, loc, flags))
            }
            UnaryOp::Not => {
                if !(self.ast.types.is_integral(p.elem) || conv::is_unscoped_enum(self, p.elem)) {
                    return None;
                }
                let x = conv::decay(self, e);
                let ty = self.ety(x);
                Some(self.builder().unary(op, x, ty, loc, ExprFlags::CANNOT_OVERFLOW))
            }
            UnaryOp::LNot => {
                if !matches!(p.kind, VectorKind::Ext | VectorKind::Generic) {
                    return None;
                }
                let x = conv::decay(self, e);
                let ty = self.signed_vector_type(self.ety(x));
                Some(self.builder().unary(op, x, ty, loc, ExprFlags::CANNOT_OVERFLOW))
            }
            _ => None,
        }
    }

    /// A built-in binary or compound-assignment operator with a vector
    /// operand (not `=`, `,`, `.*`, `->*`).
    pub(crate) fn vector_binary(&mut self, op: BinaryOp, l: ExprId, r: ExprId, loc: Loc) -> ExprId {
        let comp = op.is_compound_assignment();
        let base = if comp { compound_base(op) } else { op };
        let (lt0, rt0) = (self.ety(l), self.ety(r));
        let (mut l2, mut r2) = (l, r);
        let res = match base {
            BinaryOp::Shl | BinaryOp::Shr => self.vector_shift(&mut l2, &mut r2, loc, comp),
            BinaryOp::Mul | BinaryOp::Div | BinaryOp::Add | BinaryOp::Sub => {
                self.check_vector_operands(&mut l2, &mut r2, loc, comp, false, true)
            }
            BinaryOp::Rem | BinaryOp::And | BinaryOp::Xor | BinaryOp::Or => {
                if !self.has_integer_repr(lt0) || !self.has_integer_repr(rt0) {
                    return self.invalid_operands(l, r, loc);
                }
                let allow_bool = base != BinaryOp::Rem;
                self.check_vector_operands(&mut l2, &mut r2, loc, comp, allow_bool, true)
            }
            BinaryOp::LAnd | BinaryOp::LOr => {
                match self.check_vector_operands(&mut l2, &mut r2, loc, false, false, false) {
                    Some(_) => {
                        let lt = self.ety(l2);
                        Some(self.signed_vector_type(lt))
                    }
                    None => return self.invalid_operands(l, r, loc),
                }
            }
            BinaryOp::LT
            | BinaryOp::GT
            | BinaryOp::LE
            | BinaryOp::GE
            | BinaryOp::EQ
            | BinaryOp::NE => {
                let v = self.check_vector_operands(&mut l2, &mut r2, loc, false, true, true);
                if v.is_some() {
                    self.diagnose_self_comparison(base, l2, r2, loc);
                }
                v.map(|v| self.signed_vector_type(v))
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
            if matches!(base, BinaryOp::Div | BinaryOp::Rem) {
                // clang diagnoses division by zero for scalars only.
            }
            return self.builder().binary(op, l2, r2, ty, loc);
        }
        if !self.check_modifiable(l, loc) {
            return self.recover_err(&[l, r], loc);
        }
        self.check_null_indirection(l);
        let lhs = conv::strip_cv(self, lt0);
        if !self.ast.types.same_unqualified(ty, lhs) {
            let (x, y) = self.ty_arg_pair(lt0, ty);
            let msg = self.diag(DiagId::sema_assign_incompatible, loc).arg(x).arg(y);
            self.emit(msg);
            return self.recover_err(&[l, r], loc);
        }
        self.builder().compound_assign(op, l, r2, lt0, ty, ty, loc)
    }

    /// clang's `CheckVectorOperands` (`allow_bool`: its
    /// `AllowBoolOperation`; `report`: its `ReportInvalid`). Converts the
    /// operands in place and returns the result type, or `None` after a
    /// diagnostic (none for a rejected `bool` vector when `!report`).
    fn check_vector_operands(
        &mut self,
        l: &mut ExprId,
        r: &mut ExprId,
        loc: Loc,
        comp: bool,
        allow_bool: bool,
        report: bool,
    ) -> Option<QualType> {
        if !comp {
            *l = conv::decay(self, *l);
        }
        *r = conv::decay(self, *r);
        let lt = conv::strip_cv(self, self.ety(*l));
        let rt = conv::strip_cv(self, self.ety(*r));
        let (lv, rv) = (self.vector_parts(lt), self.vector_parts(rt));
        if !allow_bool && (self.is_ext_bool_vector(lt) || self.is_ext_bool_vector(rt)) {
            if report {
                self.vector_diag2(DiagId::sema_invalid_operands, lt, rt, loc);
            }
            return None;
        }
        if self.ast.types.same_type(lt, rt) {
            return Some(lt);
        }
        if lv.is_some() && rv.is_some() && self.compatible_vectors(lt, rt) {
            if lv.is_some_and(VecParts::is_ext) {
                *r = conv::cast_to(self, CastKind::BitCast, *r, lt, None);
                return Some(lt);
            }
            if !comp {
                *l = conv::cast_to(self, CastKind::BitCast, *l, rt, None);
            }
            return Some(rt);
        }
        // clang: an expression mixing fixed-length and sizeless SVE
        // vectors, or GNU and SVE ones, is ambiguous (it affects the ABI).
        let fixed = |v: Option<VecParts>| v.is_some_and(|p| p.kind.is_sve_fixed());
        let generic = |v: Option<VecParts>| v.is_some_and(|p| p.kind == VectorKind::Generic);
        let (ls, rs) = (self.is_sve_vector(lt), self.is_sve_vector(rt));
        let id = if (ls && fixed(rv)) || (rs && fixed(lv)) {
            Some(DiagId::sema_sve_fixed_sizeless_ambiguous)
        } else if (generic(lv) && (fixed(rv) || rs)) || (generic(rv) && (fixed(lv) || ls)) {
            Some(DiagId::sema_sve_gnu_ambiguous)
        } else {
            None
        };
        if let Some(id) = id {
            if report {
                let (x, y) = self.ty_arg_pair(lt, rt);
                let msg = self.diag(id, loc).arg(x).arg(y);
                self.emit(msg);
            }
            return None;
        }
        if let (Some(v), None) = (lv, rv) {
            let ok = if v.is_ext() {
                self.vector_convert_and_splat(Some(r), rt, v.elem, lt)
            } else {
                let vec = *l;
                self.gcc_vector_convert_and_splat(r, vec)
            };
            if ok {
                return Some(lt);
            }
        }
        if let (None, Some(v)) = (lv, rv) {
            if v.is_ext() {
                let target = if comp { None } else { Some(&mut *l) };
                if self.vector_convert_and_splat(target, lt, v.elem, rt) {
                    return Some(rt);
                }
            } else {
                let vec = *r;
                if comp || self.gcc_vector_convert_and_splat(l, vec) {
                    return Some(rt);
                }
            }
        }
        let (vec_ty, other_ty) = if lv.is_some() { (lt, rt) } else { (rt, lt) };
        if self.lax_vector_conversion(other_ty, vec_ty) {
            if !comp {
                if lv.is_some() {
                    *r = conv::cast_to(self, CastKind::BitCast, *r, vec_ty, None);
                } else {
                    *l = conv::cast_to(self, CastKind::BitCast, *l, vec_ty, None);
                }
                return Some(vec_ty);
            }
            let vt_count = lv.or(rv).map_or(0, |p| p.count);
            if self.is_vector_type(other_ty) || (self.is_scalar_real(other_ty) && vt_count == 1) {
                *r = conv::cast_to(self, CastKind::BitCast, *r, lt, None);
                return Some(vec_ty);
            }
        }
        if (rv.is_none() && !self.is_scalar_real(rt)) || (lv.is_none() && !self.is_scalar_real(lt))
        {
            self.vector_diag2(DiagId::sema_vector_not_convertible_non_scalar, lt, rt, loc);
            return None;
        }
        if rv.is_some_and(|p| !p.is_ext()) || lv.is_some_and(|p| !p.is_ext()) {
            let (scalar, vector) = if lv.is_some() { (rt, lt) } else { (lt, rt) };
            let sel = if lv.is_some() && rv.is_some() { "1" } else { "0" };
            let (x, y) = self.ty_arg_pair(scalar, vector);
            let msg =
                self.diag(DiagId::sema_vector_implicit_truncation, loc).arg(sel).arg(x).arg(y);
            self.emit(msg);
            return None;
        }
        self.vector_diag2(DiagId::sema_vector_not_convertible, lt, rt, loc);
        None
    }

    /// clang's `isRealType` (integers, unscoped enumerations, real
    /// floating types).
    pub(crate) fn is_scalar_real(&self, q: QualType) -> bool {
        self.ast.types.is_arithmetic(q) || conv::is_unscoped_enum(self, q)
    }

    /// clang's `tryVectorConvertAndSplat` (inverted: true on success):
    /// converts the scalar `scalar` of type `sty` to `elem` and splats it
    /// to the ext vector `vty`.
    fn vector_convert_and_splat(
        &mut self,
        scalar: Option<&mut ExprId>,
        sty: QualType,
        elem: QualType,
        vty: QualType,
    ) -> bool {
        let t = &self.ast.types;
        let ck = if t.is_integral(elem) {
            if !t.is_integral(sty) {
                return false;
            }
            CastKind::IntegralCast
        } else if t.is_floating(elem) {
            if t.is_floating(sty) {
                CastKind::FloatingCast
            } else if t.is_integral(sty) {
                CastKind::IntegralToFloating
            } else {
                return false;
            }
        } else {
            return false;
        };
        if let Some(s) = scalar {
            let x = conv::cast_to(self, ck, *s, elem, None);
            *s = conv::raw_cast(self, CastKind::VectorSplat, x, vty, ValueCategory::PRValue, None);
        }
        true
    }

    /// clang's `tryGCCVectorConvertAndSplat` (inverted: true on success):
    /// splats the scalar `scalar` to the type of the GNU or SVE vector `vec` when
    /// the conversion to its element type cannot truncate.
    pub(crate) fn gcc_vector_convert_and_splat(
        &mut self,
        scalar: &mut ExprId,
        vec: ExprId,
    ) -> bool {
        let sty = conv::strip_cv(self, self.ety(*scalar));
        let vty = conv::strip_cv(self, self.ety(vec));
        let Some(elem) =
            self.vector_parts(vty).map(|p| p.elem).or_else(|| self.sve_parts(vty).map(|(e, _)| e))
        else {
            return false;
        };
        if !self.is_scalar_real(elem) || !self.is_scalar_real(sty) {
            return false;
        }
        let t = &self.ast.types;
        let ck = if t.is_integral(elem) && t.is_integral(sty) {
            if self.int_would_truncate(*scalar, sty, elem) {
                return false;
            }
            Some(CastKind::IntegralCast)
        } else if t.is_integral(elem) && t.is_floating(sty) {
            if self.scalar_bits(elem) == self.scalar_bits(sty) {
                Some(CastKind::FloatingToIntegral)
            } else {
                return false;
            }
        } else if t.is_floating(elem) {
            if t.is_floating(sty) {
                if self.float_would_truncate(*scalar, sty, elem) {
                    return false;
                }
                Some(CastKind::FloatingCast)
            } else if t.is_integral(sty) {
                if self.int_to_float_would_truncate(*scalar, sty, elem) {
                    return false;
                }
                Some(CastKind::IntegralToFloating)
            } else {
                return false;
            }
        } else if conv::is_unscoped_enum(self, sty) {
            return false;
        } else {
            None
        };
        let mut x = *scalar;
        if let Some(ck) = ck {
            x = conv::cast_to(self, ck, x, elem, None);
        }
        *scalar = conv::raw_cast(self, CastKind::VectorSplat, x, vty, ValueCategory::PRValue, None);
        true
    }

    /// clang's `canConvertIntToOtherIntTy`.
    fn int_would_truncate(&mut self, e: ExprId, ity: QualType, other: QualType) -> bool {
        let order = i64::from(conv::integer_rank(self, other).unwrap_or(0))
            - i64::from(conv::integer_rank(self, ity).unwrap_or(0));
        let isigned = conv::int_signed(self, ity);
        let osigned = conv::int_signed(self, other);
        let owidth = u64::from(conv::int_width(self, other).unwrap_or(0));
        if let Some(v) = self.fold_vector_scalar(e) {
            let nbits = if isigned && v < 0 {
                128 - u64::from((!v).leading_zeros()) + 1
            } else {
                128 - u64::from(v.leading_zeros())
            };
            if order < 0 && owidth < nbits {
                return true;
            }
            return isigned != osigned && nbits > owidth;
        }
        order < 0
    }

    /// clang's `canConvertIntTyToFloatTy`.
    fn int_to_float_would_truncate(&mut self, e: ExprId, ity: QualType, fty: QualType) -> bool {
        let prec: u32 = match self.ast.types.builtin(fty) {
            Some(BuiltinType::Float) => 24,
            Some(BuiltinType::Double) => 53,
            Some(BuiltinType::Float16 | BuiltinType::Half) => 11,
            Some(BuiltinType::BFloat16) => 8,
            _ => 113,
        };
        if let Some(v) = self.fold_vector_scalar(e) {
            let mag = v.unsigned_abs();
            if mag == 0 {
                return false;
            }
            let span = 128 - mag.leading_zeros() - mag.trailing_zeros();
            return span > prec;
        }
        self.scalar_bits(ity).is_some_and(|b| b > u64::from(prec))
    }

    /// The `float`-order and truncation check of
    /// `tryGCCVectorConvertAndSplat` for a floating scalar.
    fn float_would_truncate(&mut self, e: ExprId, sty: QualType, elem: QualType) -> bool {
        let (Some(sb), Some(eb)) = (self.ast.types.builtin(sty), self.ast.types.builtin(elem))
        else {
            return true;
        };
        let order_lt = matches!(conv::float_order(self, eb, sb), conv::FloatOrder::Less);
        let cst = if self.expr_is_value_dependent(e) {
            return false;
        } else {
            self.try_fold(e)
        };
        let Some(v) = cst else { return order_lt };
        let Some(Scalar::Float { kind, bits, .. }) = self.ast.consts.scalar(v) else {
            return order_lt;
        };
        if !order_lt {
            return false;
        }
        // Exactness of the narrowing, for the formats the host has.
        match (kind, eb) {
            (FloatKind::Double, BuiltinType::Float) => {
                let d = f64::from_bits(bits as u64);
                #[allow(clippy::cast_possible_truncation, reason = "the truncation is the test")]
                let f = d as f32;
                f64::from(f).to_bits() != d.to_bits() && !d.is_nan()
            }
            _ => true,
        }
    }

    /// The integer value of a scalar operand, folded silently.
    fn fold_vector_scalar(&mut self, e: ExprId) -> Option<i128> {
        if self.expr_is_value_dependent(e) {
            return None;
        }
        self.fold_int(e)
    }

    /// clang's `checkVectorShift`.
    fn vector_shift(
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
        let lt = self.ety(*l);
        let rt = self.ety(*r);
        let (lv, rv) = (self.vector_parts(lt), self.vector_parts(rt));
        let lelem = lv.map_or_else(|| conv::strip_cv(self, lt), |p| p.elem);
        let relem = rv.map_or_else(|| conv::strip_cv(self, rt), |p| p.elem);
        if self.is_ext_bool_vector(lt) || self.is_ext_bool_vector(rt) {
            self.vector_diag2(DiagId::sema_invalid_operands, lt, rt, loc);
            return None;
        }
        let int_like =
            |s: &Self, q: QualType| s.ast.types.is_integral(q) || conv::is_unscoped_enum(s, q);
        if !int_like(self, lelem) {
            let msg = self.diag(DiagId::sema_expect_int, loc).arg(self.ty_arg(lt));
            self.emit(msg);
            return None;
        }
        if !int_like(self, relem) {
            let msg = self.diag(DiagId::sema_expect_int, loc).arg(self.ty_arg(rt));
            self.emit(msg);
            return None;
        }
        match (lv, rv) {
            (None, Some(v)) => {
                if comp {
                    return Some(rt);
                }
                let mut x = *l;
                if lelem != relem {
                    x = conv::cast_to(self, CastKind::IntegralCast, x, relem, None);
                }
                let vt = self.ast.types.vector(relem, v.count, VectorKind::Ext);
                *l = conv::raw_cast(
                    self,
                    CastKind::VectorSplat,
                    x,
                    vt,
                    ValueCategory::PRValue,
                    None,
                );
                Some(vt)
            }
            (Some(a), Some(b)) => {
                if a.count != b.count {
                    self.vector_diag2(DiagId::sema_vector_lengths_not_equal, lt, rt, loc);
                    return None;
                }
                if a.elem != b.elem && self.scalar_bits(a.elem) != self.scalar_bits(b.elem) {
                    self.vector_diag2(DiagId::sema_vector_element_sizes_not_equal, lt, rt, loc);
                }
                Some(lt)
            }
            (Some(a), None) => {
                let vt = self.ast.types.vector(relem, a.count, VectorKind::Ext);
                *r = conv::raw_cast(
                    self,
                    CastKind::VectorSplat,
                    *r,
                    vt,
                    ValueCategory::PRValue,
                    None,
                );
                Some(lt)
            }
            (None, None) => None,
        }
    }
}

/// The operator of a compound assignment (`+=` → `+`).
pub(crate) fn compound_base(op: BinaryOp) -> BinaryOp {
    match op {
        BinaryOp::MulAssign => BinaryOp::Mul,
        BinaryOp::DivAssign => BinaryOp::Div,
        BinaryOp::RemAssign => BinaryOp::Rem,
        BinaryOp::AddAssign => BinaryOp::Add,
        BinaryOp::SubAssign => BinaryOp::Sub,
        BinaryOp::ShlAssign => BinaryOp::Shl,
        BinaryOp::ShrAssign => BinaryOp::Shr,
        BinaryOp::AndAssign => BinaryOp::And,
        BinaryOp::XorAssign => BinaryOp::Xor,
        BinaryOp::OrAssign => BinaryOp::Or,
        other => other,
    }
}

impl Sema<'_, '_> {
    /// `__builtin_convertvector(e, ty)` (clang `Sema::ConvertVectorExpr`):
    /// `e` a vector, `ty` a vector type with as many elements. The operand
    /// stays unconverted, as in clang's AST.
    pub fn act_on_convert_vector(
        &mut self,
        e: ExprId,
        ty: QualType,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        if self.is_err_expr(e) || self.is_error_type(ty) {
            return self.recover(ty, &[e], loc);
        }
        let src = self.ety(e);
        let src_dep = self.ast.types.is_dependent(src);
        let dst_dep = self.ast.types.is_dependent(ty);
        let sv = self.vector_parts(src);
        if sv.is_none() && !src_dep {
            self.error(DiagId::sema_convertvector_non_vector, loc);
            return self.recover_err(&[e], loc);
        }
        let dv = self.vector_parts(ty);
        if dv.is_none() && !dst_dep {
            self.error(DiagId::sema_convertvector_non_vector_type, loc);
            return self.recover_err(&[e], loc);
        }
        if let (Some(s), Some(d)) = (sv, dv)
            && !src_dep
            && !dst_dep
            && s.count != d.count
        {
            self.error(DiagId::sema_convertvector_incompatible_vector, loc);
            return self.recover_err(&[e], loc);
        }
        self.builder().convert_vector(e, ty, loc, rparen)
    }

    /// `__builtin_shufflevector(v1, v2, i...)` (clang
    /// `Sema::BuiltinShuffleVector`; WP-282): at least two arguments; `v1`
    /// and `v2` vectors of the same unqualified type; each index an
    /// integer constant `-1` (a poison lane) or below twice the element
    /// count. The result has `v1`'s type when there are as many indices as
    /// elements, else a `vector_size` vector of the element type. The
    /// operands stay unconverted, as in clang's AST; the folded indices
    /// ride along for codegen and the Oven. Errors are at `loc`, the
    /// builtin's name (clang `getBeginLoc`). With two arguments it is the
    /// unary form `(v, mask)`: `mask` an integer vector with `v`'s element
    /// count, the result `v`'s type, the (empty) folded mask known.
    pub fn act_on_shufflevector(&mut self, args: &[ExprId], loc: Loc, rparen: Loc) -> ExprId {
        if args.iter().any(|&a| self.is_err_expr(a)) {
            return self.recover_err(args, loc);
        }
        if args.len() < 2 {
            let d =
                self.diag(DiagId::sema_call_too_few_args_at_least, rparen).arg(2).arg(args.len());
            self.emit(d);
            return self.recover_err(args, loc);
        }
        let name = || "__builtin_shufflevector".to_string();
        let dep = |s: &Self, e: ExprId| {
            s.ast.exprs.get(e).deps().contains(kiln_ast::expr::ExprDeps::TYPE)
        };
        let mut res = self.ety(args[0]);
        let mut count = None;
        if !dep(self, args[0]) && !dep(self, args[1]) {
            let (lt, rt) = (self.ety(args[0]), self.ety(args[1]));
            let (Some(l), Some(r)) = (self.vector_parts(lt), self.vector_parts(rt)) else {
                let d = self.diag(DiagId::sema_shufflevector_non_vector, loc).arg(name());
                self.emit(d);
                return self.recover_err(args, loc);
            };
            let same = self.ast.types.canonical(lt).without_quals()
                == self.ast.types.canonical(rt).without_quals();
            if args.len() == 2 {
                // The unary form: `v2` is a mask of integers with as many
                // elements as `v1`; the result has `v1`'s type.
                if !self.has_integer_repr(rt) || r.count != l.count {
                    let d = self.diag(DiagId::sema_shufflevector_incompatible_vec, loc).arg(name());
                    self.emit(d);
                    return self.recover_err(args, loc);
                }
                return self.builder().shuffle_vector(args, Some(&[]), res, loc, rparen);
            }
            if !same {
                let d = self.diag(DiagId::sema_shufflevector_incompatible_vec, loc).arg(name());
                self.emit(d);
                return self.recover_err(args, loc);
            }
            let n = u32::try_from(args.len() - 2).unwrap_or(u32::MAX);
            if l.count != n {
                // clang 22 keeps an ext_vector_type operand's kind.
                let kind = if self.clang_compat >= 22 && l.is_ext() {
                    l.kind
                } else {
                    VectorKind::Generic
                };
                res = self.ast.types.vector(l.elem, n, kind);
            }
            count = Some(i128::from(l.count));
        }
        let mut mask = Some(Vec::with_capacity(args.len() - 2));
        for &a in &args[2..] {
            if self.expr_is_value_dependent(a) || dep(self, a) {
                mask = None;
                continue;
            }
            let v = if self.ast.types.is_integral(self.ety(a)) { self.fold_int(a) } else { None };
            let Some(v) = v else {
                self.error(DiagId::sema_shufflevector_nonconstant_arg, loc);
                return self.recover_err(args, loc);
            };
            let word = if v == -1 {
                u32::MAX
            } else if let Some(n) = count
                && (v < 0 || v >= 2 * n)
            {
                self.error(DiagId::sema_shufflevector_argument_too_large, loc);
                return self.recover_err(args, loc);
            } else {
                u32::try_from(v).unwrap_or(u32::MAX - 1)
            };
            if let Some(m) = mask.as_mut() {
                m.push(word);
            }
        }
        if count.is_none() {
            mask = None;
        }
        self.builder().shuffle_vector(args, mask.as_deref(), res, loc, rparen)
    }

    /// `__builtin_reduce_{add,mul,and,or,xor,min,max}(v)` (clang
    /// `CheckBuiltinFunctionCall`, `PrepareBuiltinReduceMathOneArgCall`):
    /// one argument after the usual unary conversions, a vector (of
    /// integers, except for `min`/`max`); the call has the vector's
    /// element type.
    pub(crate) fn builtin_reduce(
        &mut self,
        id: u32,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        use kiln_ast::decl::builtin as b;
        if args.len() != 1 {
            let (d, at) = if args.is_empty() {
                (DiagId::sema_call_too_few_args, rparen)
            } else {
                (DiagId::sema_call_too_many_args, self.begin_loc(args[1]))
            };
            let msg = self.diag(d, at).arg(1).arg(args.len());
            self.emit(msg);
            return None;
        }
        let a = conv::usual_unary_conversions(self, args[0]);
        let t = self.ety(a);
        let top = self.ast.types.desugar_top(t);
        let elem = match self.ast.types.view(top.ty()) {
            TypeView::Vector { elem, .. } => Some(elem),
            _ => None,
        };
        let ints_only = !matches!(id, b::REDUCE_MIN | b::REDUCE_MAX);
        let ok =
            elem.is_some_and(|el| !ints_only || conv::rank::is_integral_or_unscoped_enum(self, el));
        let Some(elem) = elem.filter(|_| ok) else {
            let what = match (ints_only, self.clang_compat >= 22) {
                (true, true) => "vector of integer types",
                (true, false) => "vector of integers",
                (false, _) => "vector type",
            };
            let msg = self
                .diag(DiagId::sema_builtin_arg_must_be, self.begin_loc(a))
                .arg(0)
                .arg(what)
                .arg(self.ty_arg(t));
            self.emit(msg);
            return None;
        };
        Some((vec![a], elem))
    }

    /// clang's `checkMathBuiltinElementType` (no restriction): a vector, or
    /// a real type other than `bool` or an enumeration
    /// (`ConstantMatrixType::isValidElementType`).
    fn elementwise_math_type(&self, q: QualType) -> bool {
        self.is_vector_type(q)
            || (self.ast.types.is_arithmetic(q)
                && self.ast.types.builtin(q) != Some(BuiltinType::Bool))
    }

    /// `__builtin_elementwise_abs` (clang `BI__builtin_elementwise_abs`;
    /// spec/sema.md §5.4). Compat 22: the lvalue conversion only, and the
    /// (element) type must not be an unsigned integer (`bool` and an
    /// unscoped enumeration with an unsigned underlying type count). clang
    /// 22 accepts any other operand type (a pointer, a class); kiln also
    /// rejects those with the same message. Compat 19: the usual unary
    /// conversions, then a vector, integer or floating type that is not
    /// unsigned. The result has the (converted) argument's type.
    pub(crate) fn builtin_elementwise_abs(
        &mut self,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        if !self.builtin_arg_count(args, 1, rparen) {
            return None;
        }
        let new = self.clang_compat >= 22;
        let a = if new {
            conv::lvalue_to_rvalue(self, args[0])
        } else {
            conv::usual_unary_conversions(self, args[0])
        };
        let t = conv::strip_cv(self, self.ety(a));
        if self.ast.types.canonical(t).without_quals() == QualType::ERROR {
            return None;
        }
        let elem = self.vector_parts(t).map_or(t, |p| p.elem);
        let math = self.elementwise_math_type(t);
        let unsigned = self.is_unsigned_integer_type(elem);
        let what = match (new, math) {
            _ if math && !unsigned => return Some((vec![a], t)),
            (true, _) => "scalar or vector of signed integer or floating-point types",
            (false, true) => "signed integer or floating point type",
            (false, false) => "vector, integer or floating point type",
        };
        let msg = self
            .diag(DiagId::sema_builtin_arg_must_be, self.begin_loc(a))
            .arg(0)
            .arg(what)
            .arg(self.ty_arg(t));
        self.emit(msg);
        None
    }

    /// `__builtin_elementwise_max` / `_min` (clang `BuiltinElementwiseMath`;
    /// spec/sema.md §5.4). Compat 22: both arguments decay (no promotion);
    /// the first must be a vector or a non-`bool`, non-enumeration
    /// arithmetic type, and the second must have the same unqualified
    /// type. Compat 19: the usual arithmetic conversions (comparison)
    /// first, so `max(short, long)` is `long`; operands without a common
    /// type are `of different types`, then the common type is checked. The
    /// result has the (common) argument type.
    pub(crate) fn builtin_elementwise_minmax(
        &mut self,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        if !self.builtin_arg_count(args, 2, rparen) {
            return None;
        }
        let new = self.clang_compat >= 22;
        let err = |s: &Self, e: ExprId| {
            s.ast.types.canonical(s.ety(e)).without_quals() == QualType::ERROR
        };
        let (a, b, res) = if new {
            let a = conv::decay(self, args[0]);
            let b = conv::decay(self, args[1]);
            if err(self, a) || err(self, b) {
                return None;
            }
            (a, b, conv::strip_cv(self, self.ety(a)))
        } else {
            let a = conv::usual_unary_conversions(self, args[0]);
            let b = conv::usual_unary_conversions(self, args[1]);
            if err(self, a) || err(self, b) {
                return None;
            }
            let (ta, tb) = (self.ety(a), self.ety(b));
            let arith = |s: &Self, q| s.ast.types.is_arithmetic(q);
            if self.ast.types.same_unqualified(ta, tb) {
                (a, b, conv::strip_cv(self, ta))
            } else if arith(self, ta) && arith(self, tb) {
                match conv::usual_arithmetic_conversions_kind(
                    self,
                    a,
                    b,
                    conv::ArithConvKind::Comparison,
                ) {
                    Ok(r) => r,
                    Err(e) if e.id != DiagId::sema_invalid_operands => {
                        let at = self.begin_loc(a);
                        self.emit_conv_error(e, at);
                        return None;
                    }
                    Err(_) => (a, b, QualType::ERROR),
                }
            } else {
                (a, b, QualType::ERROR)
            }
        };
        let (ta, tb) = (self.ety(a), self.ety(b));
        let at = self.begin_loc(a);
        if !new && (res == QualType::ERROR || !self.ast.types.same_unqualified(ta, tb)) {
            let msg = self
                .diag(DiagId::sema_call_different_arg_types, at)
                .arg(self.ty_arg(ta))
                .arg(self.ty_arg(tb));
            self.emit(msg);
            return None;
        }
        if !self.elementwise_math_type(res) {
            let what = if new {
                "vector, integer or floating-point type"
            } else {
                "vector, integer or floating point type"
            };
            let msg = self
                .diag(DiagId::sema_builtin_arg_must_be, at)
                .arg(0)
                .arg(what)
                .arg(self.ty_arg(res));
            self.emit(msg);
            return None;
        }
        if new && !self.ast.types.same_unqualified(ta, tb) {
            let msg = self
                .diag(DiagId::sema_call_different_arg_types, at)
                .arg(self.ty_arg(ta))
                .arg(self.ty_arg(tb));
            self.emit(msg);
            return None;
        }
        Some((vec![a, b], res))
    }

    /// `__builtin_elementwise_add_sat` / `_sub_sat` (clang
    /// `BI__builtin_elementwise_add_sat`; spec/sema.md §5.4). Compat 22
    /// (`BuiltinElementwiseMath` with the integer restriction): both
    /// arguments decay (no promotion); the first must be a scalar or vector
    /// of integer type (`bool` and an unscoped enumeration count), then the
    /// second must have the same unqualified type. Compat 19: the checks of
    /// `__builtin_elementwise_max` (usual conversions first), then the
    /// (element) type must be an integer type. The result has the (common)
    /// argument type.
    pub(crate) fn builtin_elementwise_sat(
        &mut self,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        let int_elem = |s: &Self, q: QualType| {
            let elem = s.vector_parts(q).map_or(q, |p| p.elem);
            s.is_integer_type(elem)
        };
        if self.clang_compat < 22 {
            let (args, res) = self.builtin_elementwise_minmax(args, rparen)?;
            if !int_elem(self, res) {
                let a = args[0];
                let msg = self
                    .diag(DiagId::sema_builtin_arg_must_be, self.begin_loc(a))
                    .arg(0)
                    .arg("vector of integers")
                    .arg(self.ty_arg(self.ety(a)));
                self.emit(msg);
                return None;
            }
            return Some((args, res));
        }
        if !self.builtin_arg_count(args, 2, rparen) {
            return None;
        }
        let a = conv::decay(self, args[0]);
        let b = conv::decay(self, args[1]);
        let (ta, tb) = (self.ety(a), self.ety(b));
        let err =
            |s: &Self, q: QualType| s.ast.types.canonical(q).without_quals() == QualType::ERROR;
        if err(self, ta) || err(self, tb) {
            return None;
        }
        let res = conv::strip_cv(self, ta);
        let at = self.begin_loc(a);
        if !int_elem(self, res) {
            let msg = self
                .diag(DiagId::sema_builtin_arg_must_be, at)
                .arg(0)
                .arg("scalar or vector of integer types")
                .arg(self.ty_arg(res));
            self.emit(msg);
            return None;
        }
        if !self.ast.types.same_unqualified(ta, tb) {
            let msg = self
                .diag(DiagId::sema_call_different_arg_types, at)
                .arg(self.ty_arg(ta))
                .arg(self.ty_arg(tb));
            self.emit(msg);
            return None;
        }
        Some((vec![a, b], res))
    }
}

// -- vector conditionals --------------------------------------------------------

impl Sema<'_, '_> {
    /// clang's `isValidVectorForConditionalCondition`: a (GNU or ext)
    /// vector of integral elements selects lane-wise in `?:`.
    pub(crate) fn is_vector_condition(&self, q: QualType) -> bool {
        self.vector_parts(q).is_some_and(|p| self.ast.types.is_integral(p.elem))
    }

    /// `c ? t : f` with a vector condition `c` (the GNU element-wise
    /// conditional; clang `CXXCheckConditionalOperands` +
    /// `CheckVectorConditionalTypes`). The condition is not converted to
    /// `bool`: each lane of the result is `c[i] ? t[i] : f[i]`. Two vector
    /// arms must have the same type (and the condition's GNU/ext kind);
    /// one vector arm goes through `CheckVectorOperands`; two scalar arms
    /// get their common type (usual arithmetic conversions) and are
    /// splatted to a vector of the condition's lane count. The result
    /// must have the condition's lane count and lane size.
    pub(crate) fn act_on_vector_conditional(
        &mut self,
        c: ExprId,
        t: ExprId,
        f: ExprId,
        loc: Loc,
    ) -> ExprId {
        let c2 = conv::decay(self, c);
        if self.expr_is_type_dependent(t) || self.expr_is_type_dependent(f) {
            return self.builder().dependent_conditional(c2, t, f, loc);
        }
        let (tt, ft) = (self.ety(t), self.ety(f));
        let (tv, fv) = (self.ast.types.is_void(tt), self.ast.types.is_void(ft));
        if tv || fv {
            let v = if tv { t } else { f };
            let at = self.begin_loc(v);
            let throw = if self.is_throw_expr(v) { "1" } else { "0" };
            let msg = self.diag(DiagId::sema_conditional_vector_has_void, at).arg(throw);
            self.emit(msg);
            return self.recover_err(&[c, t, f], loc);
        }
        let ct = conv::strip_cv(self, self.ety(c2));
        let Some(cp) = self.vector_parts(ct) else {
            return self.recover_err(&[c, t, f], loc);
        };
        let mut t2 = conv::decay(self, t);
        let mut f2 = conv::decay(self, f);
        let lt = conv::strip_cv(self, self.ety(t2));
        let rt = conv::strip_cv(self, self.ety(f2));
        let (lv, rv) = (self.vector_parts(lt), self.vector_parts(rt));
        let res = match (lv, rv) {
            (Some(l), Some(_)) => {
                if cp.is_ext() != l.is_ext() {
                    let msg = self.diag(DiagId::sema_conditional_vector_cond_result_mismatch, loc);
                    self.emit(msg);
                    return self.recover_err(&[c, t, f], loc);
                }
                if !self.ast.types.same_type(lt, rt) {
                    self.vector_diag2(DiagId::sema_conditional_vector_mismatched, lt, rt, loc);
                    return self.recover_err(&[c, t, f], loc);
                }
                lt
            }
            (Some(_), None) | (None, Some(_)) => {
                match self.check_vector_operands(&mut t2, &mut f2, loc, false, true, true) {
                    Some(r) => r,
                    None => return self.recover_err(&[c, t, f], loc),
                }
            }
            (None, None) => match self.vector_cond_scalar_arms(cp, &mut t2, &mut f2, loc) {
                Some(r) => r,
                None => return self.recover_err(&[c, t, f], loc),
            },
        };
        let Some(rp) = self.vector_parts(res) else {
            return self.recover_err(&[c, t, f], loc);
        };
        if rp.count != cp.count {
            self.vector_diag2(DiagId::sema_conditional_vector_size, ct, res, loc);
            return self.recover_err(&[c, t, f], loc);
        }
        // clang 22 lets a `bool` vector condition select lanes of any size
        // (spec/sema.md §1.1).
        let bool_cond = self.clang_compat >= 22 && cp.elem == QualType::BOOL;
        if !bool_cond && self.scalar_bits(rp.elem) != self.scalar_bits(cp.elem) {
            self.vector_diag2(DiagId::sema_conditional_vector_element_size, ct, res, loc);
            return self.recover_err(&[c, t, f], loc);
        }
        self.builder().conditional(c2, t2, f2, res, loc)
    }

    /// `CheckVectorConditionalTypes` with two scalar arms: their common
    /// type (usual arithmetic conversions), splatted to a vector of the
    /// condition's kind and lane count. `None` after a diagnostic.
    fn vector_cond_scalar_arms(
        &mut self,
        cp: VecParts,
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
        let arith = self.ast.types.is_arithmetic(elem);
        // clang builds a vector of any element type here and then rejects
        // it by lane size; kiln has no vectors of non-arithmetic elements,
        // so a same-size one is an incompatible-operands error instead.
        if !arith && self.scalar_bits(elem) == self.scalar_bits(cp.elem) {
            let msg = self
                .diag(DiagId::sema_incompatible_cond_operands, loc)
                .arg(self.ty_arg(lt))
                .arg(self.ty_arg(rt));
            self.emit(msg);
            return None;
        }
        let kind = if cp.is_ext() { VectorKind::Ext } else { VectorKind::Generic };
        let vt = self.ast.types.vector(elem, cp.count, kind);
        if arith {
            let pr = ValueCategory::PRValue;
            *t2 = conv::raw_cast(self, CastKind::VectorSplat, *t2, vt, pr, None);
            *f2 = conv::raw_cast(self, CastKind::VectorSplat, *f2, vt, pr, None);
        }
        Some(vt)
    }

    /// `b ? t : f` with a scalar condition and a vector arm (clang
    /// `CXXCheckConditionalOperands`, "Extension: conditional operator
    /// involving vector types"): `CheckVectorOperands` without `bool`
    /// vector operations gives the result type.
    pub(crate) fn scalar_cond_vector_operands(
        &mut self,
        t: &mut ExprId,
        f: &mut ExprId,
        loc: Loc,
    ) -> Option<QualType> {
        self.check_vector_operands(t, f, loc, false, false, true)
    }
}

impl Sema<'_, '_> {
    /// The ext vector type `v.name` / `p->name` accesses: `base`'s type,
    /// or for `->` its pointee (clang `LookupMemberExpr` handles an ext
    /// vector before any class lookup).
    pub(crate) fn ext_vector_base(&self, base: ExprId, arrow: bool) -> Option<QualType> {
        let t = self.ety(base);
        let v = if arrow {
            if !self.ast.types.is_pointer(t) {
                return None;
            }
            self.pointee_sugared(t)?
        } else {
            t
        };
        self.vector_parts(v).is_some_and(VecParts::is_ext).then_some(v)
    }

    /// `base.name` or `base->name` on an ext vector `vt` (clang
    /// `CheckExtVectorComponent` and `ExtVectorElementExpr`): the
    /// selected lane, or a vector of the selected lanes named by the
    /// first ext-vector typedef of that type. An lvalue for `->`, else of
    /// `base`'s value category; `base`'s qualifiers carry over. `op_loc`
    /// (the `.` or `->`) is where a bad accessor is diagnosed.
    pub(crate) fn ext_vector_member(
        &mut self,
        base: ExprId,
        vt: QualType,
        name: kiln_source::symbol::Symbol,
        loc: Loc,
        op_loc: Loc,
        arrow: bool,
    ) -> ExprId {
        let parts = self.vector_parts(vt).expect("an ext vector");
        let text = self.ast.symbols().as_str(name).to_owned();
        if self.is_ext_bool_vector(vt) {
            // clang `CheckExtVectorComponent`: no element access on a
            // `bool` ext vector (a lane is one bit, so it has no address),
            // diagnosed at the name for every accessor; clang 19 quotes
            // the name twice (spec/sema.md §5.19).
            let arg = if self.clang_compat >= 22 { text } else { format!("'{text}'") };
            let d = self.diag(DiagId::sema_ext_vector_component_name_illegal, loc).arg(arg);
            self.emit(d);
            return self.recover_err(&[base], loc);
        }
        let lanes = match kiln_ast::expr::ext_vector_access(&text, parts.count) {
            kiln_ast::expr::ExtVectorAccess::Lanes { lanes, .. } => lanes,
            kiln_ast::expr::ExtVectorAccess::Illegal(c, at) => {
                // clang 22 points at the offending character of the name
                // (spec/sema.md §1.1); clang 19 at the operator.
                let at_loc = if self.clang_compat >= 22 { loc.with_offset(at) } else { op_loc };
                let d = self
                    .diag(DiagId::sema_ext_vector_component_name_illegal, at_loc)
                    .arg(c.to_string());
                self.emit(d);
                return self.recover_err(&[base], loc);
            }
            kiln_ast::expr::ExtVectorAccess::Exceeds => {
                let d = self
                    .diag(DiagId::sema_ext_vector_component_exceeds_length, op_loc)
                    .arg(self.ty_arg(vt));
                self.emit(d);
                return self.recover_err(&[base], loc);
            }
        };
        // The element as written in the vector type (`vecType->getElementType()`).
        let top = self.ast.types.desugar_top(vt);
        let elem = match self.ast.types.view(top.ty()) {
            TypeView::Vector { elem, .. } => elem,
            _ => parts.elem,
        };
        let n = u32::try_from(lanes.len()).unwrap_or(u32::MAX);
        let mut ty = if n == 1 {
            elem
        } else {
            let v = self.ast.types.vector(elem, n, VectorKind::Ext);
            self.ext_vector_typedefs
                .iter()
                .copied()
                .find(|&td| self.ast.decls.typedef(td).is_some_and(|x| x.underlying == v))
                .and_then(|td| self.ast.decls.get(td).ty())
                .unwrap_or(v)
        };
        let cv = self.ast.types.effective_quals(self.ast.types.canonical(vt));
        if !cv.is_empty() {
            ty = self.ast.types.qualify(ty, cv);
        }
        let (base, vk) = if arrow {
            (conv::decay(self, base), ValueCategory::LValue)
        } else {
            (base, self.ast.exprs.get(base).value_category())
        };
        self.builder().ext_vector_element(base, name, ty, vk, arrow, loc)
    }

    /// Whether `e` (looking through parentheses) is an ext-vector element
    /// access that selects some lane twice: not assignable (clang
    /// `CM_DuplicateVectorComponents`).
    pub(crate) fn has_duplicate_vector_lanes(&self, e: ExprId) -> bool {
        let e = self.ignore_parens(e);
        if self.ast.exprs.get(e).kind() != kiln_ast::expr::ExprKind::ExtVectorElementExpr {
            return false;
        }
        let base = self.ast.exprs.operand(e);
        let arrow = self.ast.exprs.get(e).has(ExprFlags::ARROW);
        let Some(p) = self.ext_vector_base(base, arrow).and_then(|v| self.vector_parts(v)) else {
            return false;
        };
        let name = self.ast.symbols().as_str(self.ast.exprs.ext_vector_accessor(e));
        matches!(
            kiln_ast::expr::ext_vector_access(name, p.count),
            kiln_ast::expr::ExtVectorAccess::Lanes { repeated: true, .. }
        )
    }
}
