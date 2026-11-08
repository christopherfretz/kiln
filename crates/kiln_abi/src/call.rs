//! Calling-convention lowering (clang `ABIInfo::computeInfo`) for the
//! two supported targets: AAPCS64 (`AArch64ABIInfo`, kind AAPCS) and
//! `SysV` x86-64 (`X86_64ABIInfo`, no AVX). The result says, for the
//! return value and each argument, how its C++ type is passed in IR.
//! See spec/codegen.md §5.
// Mirrors clang's classification functions 1:1 (long functions, arms kept
// separate as in clang, short type/offset names).
#![allow(clippy::too_many_lines, clippy::many_single_char_names, clippy::match_same_arms)]

use kiln_ast::Ast;
use kiln_ast::ids::{DeclId, QualType};
use kiln_ast::types::{BuiltinType, CallConv, TypeView, VectorKind};

use crate::irtype::{IrType, IrTypes};
use crate::layout::props;
use crate::target::Arch;

/// How one value crosses the call boundary (clang `ABIArgInfo`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ArgKind {
    /// Passed as the IR value `coerce` (equal to the natural type when
    /// there is no coercion). A literal struct `coerce` with `flatten`
    /// set is split into one IR argument per element. `offset` is the
    /// byte offset of the coerced value within the object; `align` is
    /// the `alignstack` of an AAPCS64 HFA.
    Direct {
        /// The IR type of the value.
        coerce: IrType,
        /// Byte offset within the source object.
        offset: u64,
        /// Split a struct `coerce` into separate IR arguments.
        flatten: bool,
        /// `alignstack(N)` (AAPCS64 HFAs/HVAs).
        align: Option<u64>,
    },
    /// Passed as `coerce` with `signext` (`signed`) or `zeroext`.
    Extend {
        /// The IR type of the value.
        coerce: IrType,
        /// `signext` rather than `zeroext`.
        signed: bool,
    },
    /// Passed by address: `byval(T) align N` when `byval`, else a plain
    /// pointer to a caller-owned temporary. For the return value this is
    /// the `sret` pointer, and `byval` is clang's `getIndirectByVal` (true
    /// for `getNaturalAlignIndirect` returns: no `result.ptr` spill).
    Indirect {
        /// `byval`.
        byval: bool,
        /// Alignment of the pointee.
        align: u64,
    },
    /// Not passed at all.
    Ignore,
}

/// The lowering of one value plus its `noundef`-ness.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ArgInfo {
    /// How it is passed.
    pub kind: ArgKind,
    /// Carries `noundef` (clang `DetermineNoUndef`).
    pub noundef: bool,
}

/// The lowering of a whole call (clang `CGFunctionInfo`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CallInfo {
    /// The return value; `Indirect` means `sret`.
    pub ret: ArgInfo,
    /// The arguments, required then variadic.
    pub args: Vec<ArgInfo>,
    /// Width in bits of the widest vector in the signature (x86
    /// `"min-legal-vector-width"`).
    pub max_vector_width: u64,
}

/// Classify a call returning `ret` with argument types `args`, of which
/// the first `n_required` are named parameters, to a function type of
/// calling convention `cc` (`variadic`: the prototype has an ellipsis).
/// `strict_return` is clang's `HasStrictReturn` (false for `extern "C"`
/// callees).
pub fn classify_call(
    t: &mut IrTypes<'_>,
    ret: QualType,
    args: &[QualType],
    n_required: usize,
    strict_return: bool,
    cc: CallConv,
    variadic: bool,
) -> CallInfo {
    let _ = n_required;
    let mut info = match t.target().arch() {
        Arch::AArch64 => aarch64(t, ret, args, cc == CallConv::Win64 && variadic),
        // clang `X86_64ABIInfo::computeInfo` delegates `ms_abi` to
        // `WinX86_64ABIInfo` (spec/abi.md §5.3).
        Arch::X86_64 if cc == CallConv::Win64 => win64(t, ret, args),
        Arch::X86_64 => x86_64(t, ret, args, cc == CallConv::RegCall),
    };
    let ast = t.ast();
    let ret_void = matches!(ast.types.canonical_view(ret), TypeView::Builtin(BuiltinType::Void));
    info.ret.noundef = strict_return
        && !ret_void
        && !matches!(info.ret.kind, ArgKind::Indirect { .. })
        && no_undef(t, ret, &info.ret.kind, true);
    for (a, q) in info.args.iter_mut().zip(args) {
        a.noundef = no_undef(t, *q, &a.kind, true);
    }
    let mut w = 0;
    for a in std::iter::once(&info.ret).chain(info.args.iter()) {
        if let ArgKind::Direct { coerce, .. } = &a.kind {
            w = w.max(vector_width(t, coerce));
        }
    }
    info.max_vector_width = w;
    info
}

/// How `__builtin_va_arg` reads a value of type `q` (clang's per-target
/// `EmitVAArg` classification): the variadic-argument lowering plus, on
/// x86-64, the general-purpose and SSE registers it needs (both zero when
/// it is always read from the overflow area). `AArch64` leaves them zero.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VaArgInfo {
    /// The variadic-argument lowering of the type.
    pub kind: ArgKind,
    /// x86-64 general-purpose registers needed (`neededInt`).
    pub needed_int: u32,
    /// x86-64 SSE registers needed (`neededSSE`).
    pub needed_sse: u32,
}

/// Classify `q` as an unnamed (variadic) argument read by `va_arg`.
pub fn classify_va_arg(t: &mut IrTypes<'_>, q: QualType) -> VaArgInfo {
    match t.target().arch() {
        Arch::AArch64 => VaArgInfo { kind: aarch64_arg(t, q, false), needed_int: 0, needed_sse: 0 },
        Arch::X86_64 => {
            let (kind, needed_int, needed_sse) = x86_arg(t, q, 0);
            VaArgInfo { kind, needed_int, needed_sse }
        }
    }
}

/// The `getPrimitiveSizeInBits` of a vector IR type (`<3 x i32>` is 96);
/// zero for any other type.
pub fn vector_bits(t: &mut IrTypes<'_>, ty: &IrType) -> u64 {
    match ty {
        IrType::Vector(..) => ir_bits(t, ty),
        _ => 0,
    }
}

fn vector_width(t: &mut IrTypes<'_>, ty: &IrType) -> u64 {
    match ty {
        // `getPrimitiveSizeInBits`: `<3 x i32>` is 96.
        IrType::Vector(..) => ir_bits(t, ty),
        IrType::Struct(m, _) => m.clone().iter().map(|e| vector_width(t, e)).max().unwrap_or(0),
        _ => 0,
    }
}

fn arg(kind: ArgKind) -> ArgInfo {
    ArgInfo { kind, noundef: false }
}

fn direct(coerce: IrType) -> ArgKind {
    ArgKind::Direct { coerce, offset: 0, flatten: true, align: None }
}

/// The canonical view, with the P1467 `_FloatN` types replaced by the
/// builtin they are passed as (`BuiltinType::abi_twin`).
fn view(t: &IrTypes<'_>, q: QualType) -> TypeView {
    match t.ast().types.canonical_view(q) {
        TypeView::Builtin(b) => TypeView::Builtin(b.abi_twin()),
        v => v,
    }
}

fn type_bits(t: &mut IrTypes<'_>, q: QualType) -> u64 {
    t.layouter().type_layout(q).size * 8
}

fn type_align_bits(t: &mut IrTypes<'_>, q: QualType) -> u64 {
    t.layouter().type_layout(q).align * 8
}

/// clang `getTypeUnadjustedAlignInChars` (bytes) at the compat level of
/// `t`: the AAPCS64 natural alignment that classifies a composite
/// argument and its `va_arg`, which counts the bases from clang 21 on
/// (spec/abi.md §5, spec/landed/abi/WP-583.md).
pub fn unadjusted_align(t: &mut IrTypes<'_>, q: QualType) -> u64 {
    let bases = t.clang_compat() >= 21;
    t.layouter().type_unadjusted_align(q, bases)
}

/// clang `isAggregateTypeForABI`.
fn is_aggregate(t: &IrTypes<'_>, q: QualType) -> bool {
    match view(t, q) {
        TypeView::Record(_)
        | TypeView::ConstantArray(_)
        | TypeView::IncompleteArray(_)
        | TypeView::Complex(_) => true,
        TypeView::MemberPointer { pointee, .. } => {
            matches!(t.ast().types.canonical_view(pointee), TypeView::Function(_))
        }
        // `hasScalarEvaluationKind(_Atomic(T))` is that of `T` (WP-183).
        TypeView::Atomic(v) => is_aggregate(t, v),
        _ => false,
    }
}

fn enum_int(ast: &Ast, d: DeclId) -> QualType {
    ast.decls
        .enumeration(ast.definition(d).unwrap_or(d))
        .and_then(|e| e.int_type)
        .unwrap_or(QualType::INT)
}

/// clang `isPromotableIntegerTypeForABI` (after enum stripping).
fn is_promotable(t: &IrTypes<'_>, q: QualType) -> bool {
    match view(t, q) {
        TypeView::Builtin(b) => matches!(
            b,
            BuiltinType::Bool
                | BuiltinType::Char
                | BuiltinType::SChar
                | BuiltinType::UChar
                | BuiltinType::Short
                | BuiltinType::UShort
                | BuiltinType::WChar
                | BuiltinType::Char8
                | BuiltinType::Char16
                | BuiltinType::Char32
        ),
        TypeView::BitInt { width, .. } => width < 32,
        _ => false,
    }
}

fn strip_enum(t: &IrTypes<'_>, q: QualType) -> QualType {
    match view(t, q) {
        TypeView::Enum(d) => enum_int(t.ast(), d),
        _ => q,
    }
}

fn extend(t: &mut IrTypes<'_>, q: QualType) -> ArgKind {
    let coerce = t.convert(q);
    ArgKind::Extend { coerce, signed: crate::irtype::is_signed_or_enum(t.ast(), q, t.target()) }
}

/// clang `DetermineNoUndef`.
fn no_undef(t: &mut IrTypes<'_>, q: QualType, kind: &ArgKind, check_coerce: bool) -> bool {
    let mem = t.convert_mem(q);
    match kind {
        ArgKind::Indirect { .. } | ArgKind::Extend { .. } => return true,
        ArgKind::Ignore => return false,
        ArgKind::Direct { coerce, .. } => {
            if !size_equals_store_size(&mem) {
                return false;
            }
            if check_coerce && ir_bits(t, coerce) > ir_bits(t, &mem) {
                return false;
            }
        }
    }
    match view(t, q) {
        TypeView::BitInt { .. } | TypeView::LValueRef(_) | TypeView::RValueRef(_) => true,
        TypeView::Builtin(BuiltinType::NullPtr) | TypeView::MemberPointer { .. } => false,
        TypeView::Builtin(b) if b.is_sizeless() => false,
        TypeView::Builtin(_) | TypeView::Pointer(_) | TypeView::Enum(_) => true,
        TypeView::Complex(e) | TypeView::Vector { elem: e, .. } => no_undef(t, e, kind, false),
        TypeView::ConstantArray(a) => no_undef(t, a.elem, kind, false),
        TypeView::IncompleteArray(e) => no_undef(t, e, kind, false),
        _ => false,
    }
}

fn size_equals_store_size(t: &IrType) -> bool {
    match t {
        IrType::Int(n) => n % 8 == 0,
        _ => true,
    }
}

/// `DataLayout::getTypeSizeInBits` (not alloc size).
fn ir_bits(t: &mut IrTypes<'_>, ty: &IrType) -> u64 {
    match ty {
        IrType::Int(n) => u64::from(*n),
        IrType::X86Fp80 => 80,
        IrType::Vector(e, n) => ir_bits(t, &e.clone()) * *n,
        // The known minimum (`TypeSize::isKnownGT` of a scalable size
        // against a fixed one compares the minimum).
        IrType::ScalableVector(e, n) => ir_bits(t, &e.clone()) * *n,
        _ => t.size_align(ty).0 * 8,
    }
}

// ---------------------------------------------------------------- AAPCS64

/// AAPCS64; `win_variadic` is clang's `IsWinVariadic` (an `ms_abi`
/// variadic prototype), under which no argument is an HFA/HVA.
fn aarch64(t: &mut IrTypes<'_>, ret: QualType, args: &[QualType], win_variadic: bool) -> CallInfo {
    let r = aarch64_ret(t, ret);
    let a = args.iter().map(|q| arg(aarch64_arg(t, *q, win_variadic))).collect();
    CallInfo { ret: arg(r), args: a, max_vector_width: 0 }
}

/// clang `AArch64ABIInfo::isIllegalVectorType` + `coerceIllegalVector`:
/// a vector whose element count is not a power of two, or whose size is
/// neither 64 bits nor 128 bits with several elements, goes as `i32`,
/// `<2 x i32>`, `<4 x i32>` or indirectly.
fn aarch64_illegal_vector(t: &mut IrTypes<'_>, q: QualType) -> Option<ArgKind> {
    let TypeView::Vector { count, .. } = view(t, q) else {
        return None;
    };
    let size = type_bits(t, q);
    if count.is_power_of_two() && (size == 64 || (size == 128 && count != 1)) {
        return None;
    }
    let i32v = |n| IrType::Vector(Box::new(IrType::Int(32)), n);
    Some(match size {
        0..=32 => direct(IrType::Int(32)),
        64 => direct(i32v(2)),
        128 => direct(i32v(4)),
        _ => ArgKind::Indirect { byval: false, align: t.layouter().type_layout(q).align },
    })
}

/// clang `AArch64ABIInfo::coerceIllegalVector` for a fixed-length SVE
/// vector (`arm_sve_vector_bits`, WP-810): passed and returned in the
/// scalable register type, `<vscale x 16 x i1>` for a predicate and
/// `<vscale x 128/E x T>` for data of element `T` of `E` bits.
fn aarch64_sve_fixed(t: &mut IrTypes<'_>, q: QualType) -> Option<ArgKind> {
    let TypeView::Vector { elem, kind, .. } = view(t, q) else {
        return None;
    };
    let sv = |e: IrType, n| direct(IrType::ScalableVector(Box::new(e), n));
    match kind {
        VectorKind::SveFixedPredicate => Some(sv(IrType::Int(1), 16)),
        VectorKind::SveFixedData => {
            let bits = type_bits(t, elem).max(1);
            let e = t.convert(elem);
            Some(sv(e, 128 / bits))
        }
        _ => None,
    }
}

/// clang `ABIInfo::useFirstFieldIfTransparentUnion`: an argument of a
/// union that kept `transparent_union` (WP-809) is classified as its
/// first field.
fn first_field_if_transparent(ast: &Ast, q: QualType) -> QualType {
    let TypeView::Record(d) = ast.types.canonical_view(q) else { return q };
    let def = ast.definition(d).unwrap_or(d);
    if !props::has_attr(ast, def, kiln_ast::attr::AttrKind::TRANSPARENT_UNION) {
        return q;
    }
    ast.decls
        .members(def)
        .find(|&m| ast.decls.get(m).kind() == kiln_ast::decl::DeclKind::Field)
        .and_then(|f| ast.decls.get(f).ty())
        .unwrap_or(q)
}

fn aarch64_arg(t: &mut IrTypes<'_>, q: QualType, win_variadic: bool) -> ArgKind {
    let q = first_field_if_transparent(t.ast(), q);
    if let Some(k) = aarch64_sve_fixed(t, q) {
        return k;
    }
    if let Some(k) = aarch64_illegal_vector(t, q) {
        return k;
    }
    if !is_aggregate(t, q) {
        let q = strip_enum(t, q);
        if let TypeView::BitInt { width, .. } = view(t, q)
            && width > 128
        {
            return ArgKind::Indirect { byval: false, align: t.layouter().type_layout(q).align };
        }
        return direct(t.convert(q));
    }
    if is_indirect_record(t.ast(), q) {
        return ArgKind::Indirect { byval: false, align: t.layouter().type_layout(q).align };
    }
    let size = type_bits(t, q);
    let empty = is_empty_record(t, q, true);
    if empty || size == 0 {
        // C++, non-Darwin.
        if empty && size == 0 {
            return ArgKind::Ignore;
        }
        if t.clang_compat() < 20 {
            return direct(IrType::Int(8));
        }
        // clang 20+ (GCC's convention, spec/abi.md §8.23): a zero-sized
        // argument is ignored and a non-empty-sized empty class takes the
        // generic path below (`i64`).
        if size == 0 {
            return ArgKind::Ignore;
        }
    }
    if let Some((base, members)) = homogeneous(t, q).filter(|_| !win_variadic) {
        // AAPCS64 C.4: the stack slot of an HFA/HVA is 16-byte aligned
        // only for a naturally 16-byte-aligned type (clang
        // `getTypeUnadjustedAlignInChars`; an `alignas` on the class
        // itself does not count, one on a member does).
        let align = unadjusted_align(t, q);
        let bt = t.convert(base);
        return ArgKind::Direct {
            coerce: IrType::Array(Box::new(bt), members),
            offset: 0,
            flatten: true,
            align: Some(if align >= 16 { 16 } else { 8 }),
        };
    }
    if size <= 128 {
        // AAPCS64 C.10 classifies by the natural alignment (clang
        // `getTypeUnadjustedAlign`): an `alignas(16)` pair of `i64` is
        // `[2 x i64]` in any register pair, not an even-pair `i128`.
        let a = unadjusted_align(t, q) * 8;
        let a = if a < 128 { 64 } else { 128 };
        let size = size.next_multiple_of(a);
        // clang 21+: one or two 8-byte slots of an 8-byte-aligned class
        // made only of pointers are `ptr` / `[2 x ptr]`, not integers
        // (an argument only; the return keeps `i64` / `[2 x i64]`).
        let base = if t.clang_compat() >= 21
            && (size == 64 || size == 128)
            && a == 64
            && only_pointers(t, q)
        {
            IrType::Ptr
        } else {
            IrType::Int(u32::try_from(a).unwrap_or(64))
        };
        return direct(if size == a { base } else { IrType::Array(Box::new(base), size / a) });
    }
    ArgKind::Indirect { byval: false, align: t.layouter().type_layout(q).align }
}

/// clang `AArch64ABIInfo::classifyArgumentType`'s `ContainsOnlyPointers`:
/// a non-empty class whose bases and fields (arrays looked through) are
/// all 64-bit object pointers or references into the default address
/// space, or classes of them.
fn only_pointers(t: &mut IrTypes<'_>, q: QualType) -> bool {
    if is_empty_record(t, q, true) {
        return false;
    }
    let ast = t.ast();
    let TypeView::Record(d) = view(t, q) else { return false };
    let Some(def) = ast.definition(d) else { return false };
    let bases: Vec<QualType> = ast.decls.bases(def, &ast.lists).map(|b| b.ty).collect();
    if !bases.into_iter().all(|b| only_pointers(t, b)) {
        return false;
    }
    let ptr_bits = t.target().pointer_size * 8;
    props::fields(ast, def).iter().all(|f| {
        let mut ft = f.ty;
        while let TypeView::ConstantArray(a) = view(t, ft) {
            ft = a.elem;
        }
        match view(t, ft) {
            TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => {
                ptr_bits == 64 && !matches!(view(t, p), TypeView::ExtQual { .. })
            }
            _ => only_pointers(t, ft),
        }
    })
}

fn aarch64_ret(t: &mut IrTypes<'_>, q: QualType) -> ArgKind {
    if matches!(view(t, q), TypeView::Builtin(BuiltinType::Void)) {
        return ArgKind::Ignore;
    }
    if is_indirect_record(t.ast(), q) {
        return ArgKind::Indirect { byval: false, align: t.layouter().type_layout(q).align };
    }
    if let Some(k) = aarch64_sve_fixed(t, q) {
        return k;
    }
    // Large vector types are returned via memory.
    if matches!(view(t, q), TypeView::Vector { .. }) && type_bits(t, q) > 128 {
        return ArgKind::Indirect { byval: true, align: t.layouter().type_layout(q).align };
    }
    if !is_aggregate(t, q) {
        let q = strip_enum(t, q);
        if let TypeView::BitInt { width, .. } = view(t, q)
            && width > 128
        {
            return ArgKind::Indirect { byval: true, align: t.layouter().type_layout(q).align };
        }
        return direct(t.convert(q));
    }
    let size = type_bits(t, q);
    if size == 0 || is_empty_record(t, q, true) {
        return ArgKind::Ignore;
    }
    if homogeneous(t, q).is_some() {
        return direct(t.convert(q));
    }
    if size <= 128 {
        if size <= 64 {
            return direct(IrType::Int(u32::try_from(size).unwrap_or(64)));
        }
        // clang 19-22 `classifyReturnType` reads the adjusted
        // `getTypeAlign` here (an `alignas(16)` pair returns as `i128`;
        // x0:x1 either way).
        let a = type_align_bits(t, q);
        let size = size.next_multiple_of(64);
        if a < 128 && size == 128 {
            return direct(IrType::Array(Box::new(IrType::Int(64)), 2));
        }
        return direct(IrType::Int(u32::try_from(size).unwrap_or(128)));
    }
    ArgKind::Indirect { byval: true, align: t.layouter().type_layout(q).align }
}

/// clang `isEmptyRecord(T, AllowArrays)`.
fn is_empty_record(t: &mut IrTypes<'_>, q: QualType, allow_arrays: bool) -> bool {
    let ast = t.ast();
    let TypeView::Record(d) = view(t, q) else { return false };
    let Some(def) = ast.definition(d) else { return false };
    for b in ast.decls.bases(def, &ast.lists) {
        if !is_empty_record(t, b.ty, true) {
            return false;
        }
    }
    props::fields(ast, def).iter().all(|f| is_empty_field(t, f, allow_arrays))
}

/// clang `isEmptyField`.
fn is_empty_field(t: &mut IrTypes<'_>, f: &props::FieldDesc, allow_arrays: bool) -> bool {
    if f.bit_width.is_some() && !f.named {
        return true;
    }
    let mut ft = f.ty;
    let mut was_array = false;
    if allow_arrays {
        while let TypeView::ConstantArray(a) = view(t, ft) {
            if t.ast().types.array_size(ft) == Some(0) {
                return true;
            }
            ft = a.elem;
            was_array = true;
        }
    }
    if !matches!(view(t, ft), TypeView::Record(_)) {
        return false;
    }
    if was_array || !f.nua {
        return false;
    }
    is_empty_record(t, ft, allow_arrays)
}

/// clang `isSingleElementStruct`: the one non-empty scalar a record
/// wraps (through bases, nested records and one-element arrays), when the
/// record has no padding beyond it.
fn single_element(t: &mut IrTypes<'_>, q: QualType) -> Option<QualType> {
    let ast = t.ast();
    let TypeView::Record(d) = view(t, q) else { return None };
    let def = ast.definition(d)?;
    let mut found = None;
    for b in ast.decls.bases(def, &ast.lists) {
        if is_empty_record(t, b.ty, true) {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some(single_element(t, b.ty)?);
    }
    for f in props::fields(ast, def) {
        if is_empty_field(t, &f, true) {
            continue;
        }
        if found.is_some() {
            return None;
        }
        let mut ft = f.ty;
        while let TypeView::ConstantArray(a) = view(t, ft) {
            if ast.types.array_size(ft) != Some(1) {
                break;
            }
            ft = a.elem;
        }
        found = Some(if is_aggregate(t, ft) { single_element(t, ft)? } else { ft });
    }
    let found = found?;
    (type_bits(t, found) == type_bits(t, q)).then_some(found)
}

/// clang `X86_64ABIInfo::GetByteVectorType`: the IR type of an `SSE`+`SSEUP`
/// eightbyte pair. A wrapper record of one vector or `fp128` passes as that
/// vector or `fp128`; anything else as `<N x double>`.
fn byte_vector(t: &mut IrTypes<'_>, q: QualType) -> IrType {
    let q = single_element(t, q).unwrap_or(q);
    let ir = t.convert(q);
    if matches!(ir, IrType::Vector(..) | IrType::Fp128) {
        return ir;
    }
    IrType::Vector(Box::new(IrType::Double), type_bits(t, q) / 64)
}

/// clang 20+ `AArch64ABIInfo::passAsPureScalableType` with a nonzero
/// register count (WP-810): `q` is a record (not a union, passable in
/// registers) or a nonempty array whose non-empty members are, at every
/// depth, fixed-length SVE vectors (`arm_sve_vector_bits`), at least one.
/// AAPCS64 passes such an aggregate expanded in SVE registers, which kiln
/// does not lower yet; clang 19 lowers it as any other aggregate.
pub fn is_pure_scalable(t: &mut IrTypes<'_>, q: QualType) -> bool {
    matches!(view(t, q), TypeView::Record(_) | TypeView::ConstantArray(_))
        && pure_scalable_count(t, q).is_some_and(|n| n > 0)
}

fn pure_scalable_count(t: &mut IrTypes<'_>, q: QualType) -> Option<u64> {
    let ast = t.ast();
    match view(t, q) {
        TypeView::ConstantArray(a) => {
            let n = ast.types.array_size(q).filter(|&n| n > 0)?;
            Some(pure_scalable_count(t, a.elem)? * n)
        }
        TypeView::Record(d) => {
            let def = ast.definition(d)?;
            if props::class_desc(ast, def).is_union || !can_pass_in_registers(ast, def) {
                return None;
            }
            let mut n = 0;
            for b in ast.decls.bases(def, &ast.lists) {
                if !is_empty_record(t, b.ty, true) {
                    n += pure_scalable_count(t, b.ty)?;
                }
            }
            for f in props::fields(ast, def) {
                if !is_empty_field(t, &f, true) {
                    n += pure_scalable_count(t, f.ty)?;
                }
            }
            Some(n)
        }
        TypeView::Vector { kind, .. } if kind.is_sve_fixed() => Some(1),
        _ => None,
    }
}

/// clang `isHomogeneousAggregate` with `AArch64`'s base-type and size rules.
fn homogeneous(t: &mut IrTypes<'_>, q: QualType) -> Option<(QualType, u64)> {
    let mut base = None;
    let n = homog(t, q, &mut base)?;
    let b = base?;
    (n > 0 && n <= 4).then_some((b, n))
}

fn homog(t: &mut IrTypes<'_>, q: QualType, base: &mut Option<QualType>) -> Option<u64> {
    let ast = t.ast();
    match view(t, q) {
        TypeView::ConstantArray(a) => {
            let n = ast.types.array_size(q)?;
            if n == 0 {
                return None;
            }
            Some(homog(t, a.elem, base)? * n)
        }
        TypeView::Record(d) => {
            let def = ast.definition(d)?;
            let is_union = props::class_desc(ast, def).is_union;
            if props::class_desc(ast, def).dynamic {
                return None;
            }
            let mut members = 0u64;
            for b in ast.decls.bases(def, &ast.lists) {
                if is_empty_record(t, b.ty, true) {
                    continue;
                }
                members += homog(t, b.ty, base)?;
            }
            for f in props::fields(ast, def) {
                let mut ft = f.ty;
                while let TypeView::ConstantArray(a) = view(t, ft) {
                    if ast.types.array_size(ft) == Some(0) {
                        return None;
                    }
                    ft = a.elem;
                }
                if is_empty_record(t, ft, true) {
                    continue;
                }
                if f.bit_width == Some(0) {
                    continue;
                }
                let m = homog(t, f.ty, base)?;
                members = if is_union { members.max(m) } else { members + m };
            }
            let b = (*base)?;
            if type_bits(t, b) * members != type_bits(t, q) {
                return None;
            }
            Some(members)
        }
        v => {
            let (mut members, mut ty, mut tv) = (1, q, v);
            if let TypeView::Complex(e) = tv {
                members = 2;
                ty = e;
                tv = view(t, e);
            }
            let ok = match tv {
                TypeView::Builtin(b) => matches!(
                    b,
                    BuiltinType::Half
                        | BuiltinType::Float16
                        | BuiltinType::BFloat16
                        | BuiltinType::Float
                        | BuiltinType::Double
                        | BuiltinType::LongDouble
                        | BuiltinType::Float128
                ),
                // clang 20+ `isHomogeneousAggregateBaseType`: a
                // fixed-length SVE vector is no HFA/HVA base (WP-810).
                TypeView::Vector { kind, .. } if kind.is_sve_fixed() && t.clang_compat() >= 20 => {
                    false
                }
                TypeView::Vector { .. } => {
                    let s = type_bits(t, ty);
                    s == 64 || s == 128
                }
                _ => false,
            };
            if !ok {
                return None;
            }
            match *base {
                None => *base = Some(ty),
                Some(b) => {
                    let bv = matches!(view(t, b), TypeView::Vector { .. });
                    let tvv = matches!(tv, TypeView::Vector { .. });
                    if bv != tvv || type_bits(t, b) != type_bits(t, ty) {
                        return None;
                    }
                }
            }
            Some(members)
        }
    }
}

// ---------------------------------------------------------------- x86-64

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(clippy::enum_variant_names, reason = "clang's X86_64ABIInfo::Class names")]
enum Class {
    Integer,
    Sse,
    SseUp,
    X87,
    X87Up,
    ComplexX87,
    NoClass,
    Memory,
}

fn merge(accum: Class, field: Class) -> Class {
    use Class::{ComplexX87, Integer, Memory, NoClass, Sse, X87, X87Up};
    if accum == field || field == NoClass {
        return accum;
    }
    if field == Memory {
        return Memory;
    }
    if accum == NoClass {
        return field;
    }
    if accum == Integer || field == Integer {
        return Integer;
    }
    if matches!(field, X87 | X87Up | ComplexX87) || matches!(accum, X87 | X87Up) {
        return Memory;
    }
    Sse
}

fn post_merge(size: u64, lo: &mut Class, hi: &mut Class) {
    use Class::{Memory, Sse, SseUp, X87, X87Up};
    if *hi == Memory {
        *lo = Memory;
    }
    if *hi == X87Up && *lo != X87 {
        *lo = Memory;
    }
    if size > 128 && (*lo != Sse || *hi != SseUp) {
        *lo = Memory;
    }
    if *hi == SseUp && *lo != Sse {
        *hi = Sse;
    }
}

fn classify(t: &mut IrTypes<'_>, q: QualType, off: u64) -> (Class, Class) {
    use Class::{ComplexX87, Integer, Memory, NoClass, Sse, SseUp, X87, X87Up};
    let mut lo = NoClass;
    let mut hi = NoClass;
    let lo_cur = off < 64;
    macro_rules! set_cur {
        ($c:expr) => {
            if lo_cur { lo = $c } else { hi = $c }
        };
    }
    set_cur!(Memory);
    let ast = t.ast();
    match view(t, q) {
        TypeView::Builtin(b) => match b {
            BuiltinType::Void => set_cur!(NoClass),
            BuiltinType::Int128 | BuiltinType::UInt128 => {
                lo = Integer;
                hi = Integer;
            }
            BuiltinType::Float
            | BuiltinType::Double
            | BuiltinType::Float16
            | BuiltinType::BFloat16 => {
                set_cur!(Sse);
            }
            BuiltinType::Float128 => {
                lo = Sse;
                hi = SseUp;
            }
            BuiltinType::LongDouble => {
                lo = X87;
                hi = X87Up;
            }
            BuiltinType::NullPtr => set_cur!(Integer),
            b if b.is_integral() => set_cur!(Integer),
            _ => {}
        },
        TypeView::Enum(d) => return classify(t, enum_int(ast, d), off),
        TypeView::Pointer(_)
        | TypeView::LValueRef(_)
        | TypeView::RValueRef(_)
        | TypeView::Function(_) => {
            set_cur!(Integer);
        }
        TypeView::MemberPointer { pointee, .. } => {
            if matches!(ast.types.canonical_view(pointee), TypeView::Function(_)) {
                lo = Integer;
                hi = Integer;
            } else {
                set_cur!(Integer);
            }
        }
        TypeView::BitInt { width, .. } => {
            if width <= 64 {
                set_cur!(Integer);
            } else if width <= 128 {
                lo = Integer;
                hi = Integer;
            }
        }
        TypeView::Complex(e) => {
            let size = type_bits(t, q);
            let esize = type_bits(t, e);
            match view(t, e) {
                TypeView::Builtin(
                    BuiltinType::Float | BuiltinType::Float16 | BuiltinType::BFloat16,
                ) => {
                    set_cur!(Sse);
                }
                TypeView::Builtin(BuiltinType::Double) => {
                    lo = Sse;
                    hi = Sse;
                }
                TypeView::Builtin(BuiltinType::LongDouble) => set_cur!(ComplexX87),
                _ => {
                    if size <= 64 {
                        set_cur!(Integer);
                    } else if size <= 128 {
                        lo = Integer;
                        hi = Integer;
                    }
                }
            }
            if hi == NoClass && off / 64 != (off + esize) / 64 {
                hi = lo;
            }
        }
        // clang `X86_64ABIInfo::classify` for vectors (no AVX: the native
        // vector size is 128 bits).
        TypeView::Vector { elem, .. } => {
            let size = type_bits(t, q);
            if matches!(size, 1 | 8 | 16 | 32) {
                set_cur!(Integer);
                if off / 64 != (off + size - 1) / 64 {
                    hi = lo;
                }
            } else if size == 64 {
                if matches!(view(t, elem), TypeView::Builtin(BuiltinType::Double)) {
                    return (lo, hi);
                }
                set_cur!(Sse);
                if off != 0 && off != 64 {
                    hi = lo;
                }
            } else if size == 128 {
                lo = Sse;
                hi = SseUp;
            }
        }
        TypeView::ConstantArray(a) => {
            let size = type_bits(t, q);
            if size > 512 {
                return (lo, hi);
            }
            if !off.is_multiple_of(type_align_bits(t, a.elem)) {
                return (lo, hi);
            }
            set_cur!(NoClass);
            let n = ast.types.array_size(q).unwrap_or(0);
            // clang: `Size > 128 && (Size != elem size || Size > native
            // vector size)`; without AVX the native vector size is 128, so
            // the condition reduces to `Size > 128`.
            if size > 128 {
                return (lo, hi);
            }
            let esize = type_bits(t, a.elem);
            let mut o = off;
            for _ in 0..n {
                let (fl, fh) = classify(t, a.elem, o);
                lo = merge(lo, fl);
                hi = merge(hi, fh);
                if lo == Memory || hi == Memory {
                    break;
                }
                o += esize;
            }
            post_merge(size, &mut lo, &mut hi);
        }
        TypeView::Record(d) => {
            let size = type_bits(t, q);
            if size > 512 {
                return (lo, hi);
            }
            let Some(def) = ast.definition(d) else { return (lo, hi) };
            if !can_pass_in_registers(ast, def) {
                return (lo, hi);
            }
            let layout = t.layouter().record(def).clone();
            set_cur!(NoClass);
            for (i, b) in ast.decls.bases(def, &ast.lists).enumerate() {
                let boff = layout.bases.get(i).map_or(0, |x| x.1 * 8);
                let (fl, fh) = classify(t, b.ty, off + boff);
                lo = merge(lo, fl);
                hi = merge(hi, fh);
                if lo == Memory || hi == Memory {
                    post_merge(size, &mut lo, &mut hi);
                    return (lo, hi);
                }
            }
            for (i, f) in props::fields(ast, def).iter().enumerate() {
                let foff = off + layout.fields.get(i).copied().unwrap_or(0);
                if f.bit_width.is_some() && !f.named {
                    continue;
                }
                // clang: `Size > 128 && ((!IsUnion && Size != field size) ||
                // Size > native vector size)`; 128 without AVX.
                if size > 128 {
                    lo = Memory;
                    post_merge(size, &mut lo, &mut hi);
                    return (lo, hi);
                }
                let in_memory = !foff.is_multiple_of(type_align_bits(t, f.ty));
                if f.bit_width.is_none() && in_memory {
                    lo = Memory;
                    post_merge(size, &mut lo, &mut hi);
                    return (lo, hi);
                }
                let (fl, fh) = if let Some(w) = f.bit_width {
                    let eb_lo = foff / 64;
                    let eb_hi = (foff + w).saturating_sub(1) / 64;
                    if eb_lo != 0 {
                        (NoClass, Integer)
                    } else {
                        (Integer, if eb_hi != 0 { Integer } else { NoClass })
                    }
                } else {
                    classify(t, f.ty, foff)
                };
                lo = merge(lo, fl);
                hi = merge(hi, fh);
                if lo == Memory || hi == Memory {
                    break;
                }
            }
            post_merge(size, &mut lo, &mut hi);
        }
        _ => {}
    }
    (lo, hi)
}

/// `SysV` x86-64; `regcall` is clang's `IsRegCall` (Intel `regcall`:
/// 11 integer and 16 SSE registers, classes classified by
/// [`regcall_struct`]).
fn x86_64(t: &mut IrTypes<'_>, ret: QualType, args: &[QualType], regcall: bool) -> CallInfo {
    let mut free_int = if regcall { 11u32 } else { 6 };
    let mut free_sse = if regcall { 16u32 } else { 8 };
    let natural_indirect = |t: &mut IrTypes<'_>, q| ArgKind::Indirect {
        byval: true,
        align: t.layouter().type_layout(q).align,
    };
    let r = if regcall && !is_indirect_record(t.ast(), ret) && is_struct_or_class(t, ret) {
        match regcall_struct(t, ret, false) {
            Some((k, ni, ns)) if free_int >= ni && free_sse >= ns => {
                free_int -= ni;
                free_sse -= ns;
                k
            }
            _ => natural_indirect(t, ret),
        }
    } else if regcall
        && let TypeView::Complex(e) = view(t, ret)
        && matches!(view(t, e), TypeView::Builtin(BuiltinType::LongDouble))
    {
        // clang: "Complex Long Double Type is passed in Memory when
        // Regcall calling convention is used."
        natural_indirect(t, ret)
    } else {
        x86_ret(t, ret)
    };
    if matches!(r, ArgKind::Indirect { .. }) {
        free_int -= 1;
    }
    let mut out = Vec::with_capacity(args.len());
    for &q in args {
        let (mut k, ni, ns) = if regcall && is_struct_or_class(t, q) {
            regcall_struct(t, q, true).unwrap_or_else(|| (natural_indirect(t, q), 0, 0))
        } else {
            x86_arg(t, q, free_int)
        };
        if free_int >= ni && free_sse >= ns {
            free_int -= ni;
            free_sse -= ns;
        } else {
            k = x86_indirect(t, q, free_int);
        }
        out.push(arg(k));
    }
    CallInfo { ret: arg(r), args: out, max_vector_width: 0 }
}

/// clang `isStructureOrClassType`.
fn is_struct_or_class(t: &IrTypes<'_>, q: QualType) -> bool {
    let TypeView::Record(d) = view(t, q) else { return false };
    let ast = t.ast();
    let def = ast.definition(d).unwrap_or(d);
    ast.decls.record(def).is_some_and(|r| r.tag != kiln_ast::decl::TagKind::Union)
}

/// clang `X86_64ABIInfo::classifyRegCallStructType`: the class `q`
/// passed (`arg`) or returned directly as its natural struct, one IR
/// argument per element, with the integer and SSE registers its fields
/// need; None (indirect at its natural alignment) for a class with a
/// flexible array member, a dynamic class, or one with a base or field
/// that is indirect.
fn regcall_struct(t: &mut IrTypes<'_>, q: QualType, arg: bool) -> Option<(ArgKind, u32, u32)> {
    let (mut ni, mut ns) = (0, 0);
    if !regcall_struct_impl(t, q, &mut ni, &mut ns) {
        return None;
    }
    let natural = t.convert(q);
    let kind = match (&natural, arg) {
        // The class's own named struct, flattened (clang leaves the
        // coerce type null, so the prologue's GEPs name `%struct.S`).
        (IrType::Record(_), true) => {
            ArgKind::Direct { coerce: natural.clone(), offset: 0, flatten: true, align: None }
        }
        _ => direct(natural),
    };
    Some((kind, ni, ns))
}

/// clang `classifyRegCallStructTypeImpl`: false when `q` is indirect,
/// else the registers its bases and fields need added to `ni`/`ns`.
fn regcall_struct_impl(t: &mut IrTypes<'_>, q: QualType, ni: &mut u32, ns: &mut u32) -> bool {
    use kiln_ast::decl::DefinitionBits as B;
    let ast = t.ast();
    let TypeView::Record(d) = view(t, q) else { return false };
    let Some(def) = ast.definition(d) else { return false };
    let Some(r) = ast.decls.record(def) else { return false };
    if r.defn.contains(B::HAS_FLEXIBLE_ARRAY_MEMBER) || r.defn.is_dynamic() {
        return false;
    }
    let bases: Vec<QualType> = ast.decls.bases(def, &ast.lists).map(|b| b.ty).collect();
    for b in bases {
        if !regcall_struct_impl(t, b, ni, ns) {
            return false;
        }
    }
    for f in props::fields(ast, def) {
        if is_struct_or_class(t, f.ty) {
            if !regcall_struct_impl(t, f.ty, ni, ns) {
                return false;
            }
            continue;
        }
        let (k, li, ls) = x86_arg(t, f.ty, u32::MAX);
        if matches!(k, ArgKind::Indirect { .. }) {
            return false;
        }
        *ni += li;
        *ns += ls;
    }
    true
}

/// clang `WinX86_64ABIInfo::computeInfo` (no `vectorcall`/`regcall`),
/// as `ms_abi` forces on an x86-64 `SysV` target.
fn win64(t: &mut IrTypes<'_>, ret: QualType, args: &[QualType]) -> CallInfo {
    let r = if is_indirect_record(t.ast(), ret) {
        ArgKind::Indirect { byval: false, align: t.layouter().type_layout(ret).align }
    } else {
        win64_classify(t, ret, true)
    };
    let a = args.iter().map(|&q| arg(win64_classify(t, q, false))).collect();
    CallInfo { ret: arg(r), args: a, max_vector_width: 0 }
}

/// clang `WinX86_64ABIInfo::classify`: "Any argument that doesn't fit in
/// 8 bytes, or is not 1, 2, 4, or 8 bytes, must be passed by reference."
fn win64_classify(t: &mut IrTypes<'_>, q: QualType, is_ret: bool) -> ArgKind {
    if matches!(view(t, q), TypeView::Builtin(BuiltinType::Void)) {
        return ArgKind::Ignore;
    }
    let q = strip_enum(t, q);
    let width = type_bits(t, q);
    let align = t.layouter().type_layout(q).align;
    let by_ref = ArgKind::Indirect { byval: false, align };
    let rec = match view(t, q) {
        TypeView::Record(d) => Some(d),
        _ => None,
    };
    if let Some(d) = rec {
        use kiln_ast::decl::DefinitionBits as B;
        if !is_ret && is_indirect_record(t.ast(), q) {
            return by_ref;
        }
        let ast = t.ast();
        let def = ast.definition(d).unwrap_or(d);
        if ast.decls.record(def).is_some_and(|r| r.defn.contains(B::HAS_FLEXIBLE_ARRAY_MEMBER)) {
            return by_ref;
        }
    }
    let member_ptr = matches!(view(t, q), TypeView::MemberPointer { .. });
    if member_ptr {
        let ir = t.convert(q);
        if matches!(ir, IrType::Ptr | IrType::Int(_)) {
            return direct(ir);
        }
    }
    if rec.is_some() || member_ptr || matches!(view(t, q), TypeView::Complex(_)) {
        if width > 64 || !width.is_power_of_two() {
            return by_ref;
        }
        return direct(IrType::Int(u32::try_from(width).unwrap_or(64)));
    }
    match view(t, q) {
        TypeView::Builtin(BuiltinType::Bool) => extend(t, q),
        TypeView::Builtin(BuiltinType::Int128 | BuiltinType::UInt128) => {
            if is_ret {
                direct(IrType::Vector(Box::new(IrType::Int(64)), 2))
            } else {
                by_ref
            }
        }
        TypeView::BitInt { .. } if width > 64 => by_ref,
        _ => direct(t.convert(q)),
    }
}

fn x86_indirect(t: &mut IrTypes<'_>, q: QualType, free_int: u32) -> ArgKind {
    let q = first_field_if_transparent(t.ast(), q);
    if is_indirect_record(t.ast(), q) {
        return ArgKind::Indirect { byval: false, align: t.layouter().type_layout(q).align };
    }
    // clang `IsIllegalVectorType`: at most 64 or over 128 bits (no AVX).
    let illegal_vector = matches!(view(t, q), TypeView::Vector { .. }) && {
        let s = type_bits(t, q);
        s <= 64 || s > 128
    };
    if !is_aggregate(t, q) && !illegal_vector && !matches!(view(t, q), TypeView::BitInt { .. }) {
        let q = strip_enum(t, q);
        return if is_promotable(t, q) { extend(t, q) } else { direct(t.convert(q)) };
    }
    let align = t.layouter().type_layout(q).align.max(8);
    if free_int == 0 {
        let size = type_bits(t, q);
        if align == 8 && size <= 64 {
            return direct(IrType::Int(u32::try_from(size).unwrap_or(64)));
        }
    }
    ArgKind::Indirect { byval: true, align }
}

fn x86_arg(t: &mut IrTypes<'_>, q: QualType, free_int: u32) -> (ArgKind, u32, u32) {
    use Class::{ComplexX87, Integer, Memory, NoClass, Sse, SseUp, X87, X87Up};
    let q = first_field_if_transparent(t.ast(), q);
    let (lo, hi) = classify(t, q, 0);
    let mut ni = 0;
    let mut ns = 0;
    let natural = t.convert(q);
    let mut res: Option<IrType> = None;
    match lo {
        NoClass => {
            if hi == NoClass {
                return (ArgKind::Ignore, 0, 0);
            }
        }
        Memory | X87 | ComplexX87 => {
            let ni = u32::from(is_indirect_record(t.ast(), q));
            return (x86_indirect(t, q, free_int), ni, 0);
        }
        SseUp | X87Up => {}
        Integer => {
            ni += 1;
            let r = integer_at(t, &natural, 0, q, 0);
            if hi == NoClass && r.is_int() {
                let s = strip_enum(t, q);
                if is_integral_or_enum(t, s) && is_promotable(t, s) {
                    return (extend(t, s), ni, ns);
                }
            }
            // clang 20+: an `i128` at offset 0 is passed directly in the
            // two registers (spec/abi.md §8.25).
            if r == IrType::Int(128) {
                return (direct(r), ni + 1, ns);
            }
            res = Some(r);
        }
        Sse => {
            ns += 1;
            res = Some(sse_at(t, &natural, 0, q, 0));
        }
    }
    let mut high = None;
    match hi {
        Integer => {
            ni += 1;
            let h = integer_at(t, &natural, 8, q, 8);
            if lo == NoClass {
                return (
                    ArgKind::Direct { coerce: h, offset: 8, flatten: true, align: None },
                    ni,
                    ns,
                );
            }
            high = Some(h);
        }
        X87Up | Sse => {
            ns += 1;
            let h = sse_at(t, &natural, 8, q, 8);
            if lo == NoClass {
                return (
                    ArgKind::Direct { coerce: h, offset: 8, flatten: true, align: None },
                    ni,
                    ns,
                );
            }
            high = Some(h);
        }
        SseUp => res = Some(byte_vector(t, q)),
        _ => {}
    }
    let res = match (res, high) {
        (Some(l), Some(h)) => pair(t, l, h),
        (Some(l), None) => l,
        (None, _) => natural,
    };
    (direct(res), ni, ns)
}

/// A `_BitInt(N)` with `N > 128`, which x86-64 passes and returns in memory.
fn is_wide_bit_int(t: &IrTypes<'_>, q: QualType) -> bool {
    matches!(view(t, q), TypeView::BitInt { width, .. } if width > 128)
}

fn x86_ret(t: &mut IrTypes<'_>, q: QualType) -> ArgKind {
    use Class::{ComplexX87, Integer, Memory, NoClass, Sse, SseUp, X87, X87Up};
    if is_indirect_record(t.ast(), q) {
        return ArgKind::Indirect { byval: false, align: t.layouter().type_layout(q).align };
    }
    let (lo, hi) = classify(t, q, 0);
    let natural = t.convert(q);
    let mut res: Option<IrType> = None;
    match lo {
        NoClass => {
            if hi == NoClass {
                return ArgKind::Ignore;
            }
        }
        Memory => {
            // clang `getIndirectReturnResult`: a `_BitInt` wider than 128
            // bits returns in memory like an aggregate.
            if !is_aggregate(t, q) && !is_wide_bit_int(t, q) {
                let s = strip_enum(t, q);
                return if is_promotable(t, s) { extend(t, s) } else { direct(natural) };
            }
            return ArgKind::Indirect { byval: true, align: t.layouter().type_layout(q).align };
        }
        Integer => {
            let r = integer_at(t, &natural, 0, q, 0);
            if hi == NoClass && r.is_int() {
                let s = strip_enum(t, q);
                if is_integral_or_enum(t, s) && is_promotable(t, s) {
                    return extend(t, s);
                }
            }
            // clang 20+ returns an `i128` at offset 0 directly (§8.25).
            if r == IrType::Int(128) {
                return direct(r);
            }
            res = Some(r);
        }
        Sse => res = Some(sse_at(t, &natural, 0, q, 0)),
        X87 => res = Some(IrType::X86Fp80),
        ComplexX87 => res = Some(IrType::Struct(vec![IrType::X86Fp80, IrType::X86Fp80], false)),
        SseUp | X87Up => {}
    }
    let mut high = None;
    match hi {
        Integer => {
            let h = integer_at(t, &natural, 8, q, 8);
            if lo == NoClass {
                return ArgKind::Direct { coerce: h, offset: 8, flatten: true, align: None };
            }
            high = Some(h);
        }
        Sse => {
            let h = sse_at(t, &natural, 8, q, 8);
            if lo == NoClass {
                return ArgKind::Direct { coerce: h, offset: 8, flatten: true, align: None };
            }
            high = Some(h);
        }
        SseUp => res = Some(byte_vector(t, q)),
        X87Up if lo != X87 => {
            let h = sse_at(t, &natural, 8, q, 8);
            if lo == NoClass {
                return ArgKind::Direct { coerce: h, offset: 8, flatten: true, align: None };
            }
            high = Some(h);
        }
        _ => {}
    }
    let res = match (res, high) {
        (Some(l), Some(h)) => pair(t, l, h),
        (Some(l), None) => l,
        (None, _) => natural,
    };
    direct(res)
}

fn is_integral_or_enum(t: &IrTypes<'_>, q: QualType) -> bool {
    match view(t, q) {
        TypeView::Builtin(b) => b.is_integral(),
        TypeView::BitInt { .. } | TypeView::Enum(_) => true,
        _ => false,
    }
}

/// clang `GetX86_64ByValArgumentPair`.
fn pair(t: &mut IrTypes<'_>, lo: IrType, hi: IrType) -> IrType {
    let (lo_size, _) = t.size_align(&lo);
    let (_, hi_align) = t.size_align(&hi);
    let hi_start = lo_size.next_multiple_of(hi_align);
    let lo = if hi_start == 8 {
        lo
    } else if matches!(lo, IrType::Half | IrType::Float) {
        IrType::Double
    } else {
        IrType::Int(64)
    };
    IrType::Struct(vec![lo, hi], false)
}

/// The struct element containing byte `off` and the offset within it.
fn element_at(
    t: &mut IrTypes<'_>,
    members: &[IrType],
    packed: bool,
    off: u64,
) -> Option<(IrType, u64)> {
    let offs = t.struct_offsets(members, packed);
    let i = offs.partition_point(|&o| o <= off).checked_sub(1)?;
    Some((members[i].clone(), off - offs[i]))
}

fn struct_body(t: &mut IrTypes<'_>, ty: &IrType) -> Option<(Vec<IrType>, bool)> {
    match ty {
        IrType::Struct(m, p) => Some((m.clone(), *p)),
        IrType::Record(d) => {
            let r = t.record(*d);
            Some((r.members.clone(), r.packed))
        }
        IrType::RecordBase(d) => {
            let r = t.record(*d);
            Some((r.base_members.clone().unwrap_or_else(|| r.members.clone()), r.packed))
        }
        _ => None,
    }
}

/// clang `GetINTEGERTypeAtOffset`.
fn integer_at(
    t: &mut IrTypes<'_>,
    ir: &IrType,
    ir_off: u64,
    src: QualType,
    src_off: u64,
) -> IrType {
    if ir_off == 0 {
        if matches!(ir, IrType::Ptr | IrType::Int(64)) {
            return ir.clone();
        }
        if let IrType::Int(w @ (8 | 16 | 32)) = ir
            && bits_no_user_data(t, src, src_off * 8 + u64::from(*w), src_off * 8 + 64)
        {
            return ir.clone();
        }
    }
    if let Some((m, p)) = struct_body(t, ir) {
        let (size, _) = t.size_align(ir);
        if ir_off < size
            && let Some((e, o)) = element_at(t, &m, p, ir_off)
        {
            return integer_at(t, &e, o, src, src_off);
        }
    }
    if let IrType::Array(e, _) = ir {
        let (es, _) = t.size_align(e);
        let eo = ir_off.checked_div(es).map_or(0, |q| q * es);
        return integer_at(t, e, ir_off - eo, src, src_off);
    }
    // clang 20+ (LLVM 20 "Pass __int128 as i128"): a 128-bit integer is
    // passed whole, so a stack-passed one gets `i128`'s 16-byte slot
    // (spec/abi.md §8.25). clang 19 splits it into two `i64`s.
    if ir_off == 0 && *ir == IrType::Int(128) && t.clang_compat() >= 20 {
        return ir.clone();
    }
    let size = t.layouter().type_layout(src).size;
    IrType::Int(u32::try_from(size.saturating_sub(src_off).min(8) * 8).unwrap_or(64))
}

/// clang `getFPTypeAtOffset`.
fn fp_at(t: &mut IrTypes<'_>, ir: &IrType, off: u64) -> Option<IrType> {
    if off == 0 && ir.is_fp() {
        return Some(ir.clone());
    }
    if let Some((m, p)) = struct_body(t, ir) {
        let (e, o) = element_at(t, &m, p, off)?;
        return fp_at(t, &e, o);
    }
    if let IrType::Array(e, _) = ir {
        let (es, _) = t.size_align(e);
        let o = off.checked_rem(es).unwrap_or(0);
        return fp_at(t, e, o);
    }
    None
}

fn is_16bit_fp(t: &IrType) -> bool {
    matches!(t, IrType::Half | IrType::BFloat)
}

/// clang `GetSSETypeAtOffset`.
fn sse_at(t: &mut IrTypes<'_>, ir: &IrType, ir_off: u64, src: QualType, src_off: u64) -> IrType {
    let source_size = t.layouter().type_layout(src).size.saturating_sub(src_off);
    let Some(t0) = fp_at(t, ir, ir_off) else { return IrType::Double };
    if t0 == IrType::Double {
        return IrType::Double;
    }
    let (t0_size, _) = t.size_align(&t0);
    let mut t1 = if source_size > t0_size { fp_at(t, ir, ir_off + t0_size) } else { None };
    if t1.is_none() {
        if is_16bit_fp(&t0) && source_size > 4 {
            t1 = fp_at(t, ir, ir_off + 4);
        }
        if t1.is_none() {
            return t0;
        }
    }
    let t1 = t1.unwrap_or(IrType::Double);
    if t0 == IrType::Float && t1 == IrType::Float {
        return IrType::Vector(Box::new(t0), 2);
    }
    if is_16bit_fp(&t0) && is_16bit_fp(&t1) {
        let t2 = if source_size > 4 { fp_at(t, ir, ir_off + 4) } else { None };
        return IrType::Vector(Box::new(t0), if t2.is_none() { 2 } else { 4 });
    }
    if is_16bit_fp(&t0) || is_16bit_fp(&t1) {
        return IrType::Vector(Box::new(IrType::Half), 4);
    }
    IrType::Double
}

/// clang `BitsContainNoUserData`.
fn bits_no_user_data(t: &mut IrTypes<'_>, q: QualType, start: u64, end: u64) -> bool {
    let size = type_bits(t, q);
    if size <= start {
        return true;
    }
    let ast = t.ast();
    match view(t, q) {
        TypeView::ConstantArray(a) => {
            let es = type_bits(t, a.elem);
            let n = ast.types.array_size(q).unwrap_or(0);
            for i in 0..n {
                let eo = i * es;
                if eo >= end {
                    break;
                }
                let est = start.saturating_sub(eo);
                if !bits_no_user_data(t, a.elem, est, end - eo) {
                    return false;
                }
            }
            true
        }
        TypeView::Record(d) => {
            let Some(def) = ast.definition(d) else { return false };
            let layout = t.layouter().record(def).clone();
            for (i, b) in ast.decls.bases(def, &ast.lists).enumerate() {
                let bo = layout.bases.get(i).map_or(0, |x| x.1 * 8);
                if bo >= end {
                    continue;
                }
                if !bits_no_user_data(t, b.ty, start.saturating_sub(bo), end - bo) {
                    return false;
                }
            }
            for (i, f) in props::fields(ast, def).iter().enumerate() {
                let fo = layout.fields.get(i).copied().unwrap_or(0);
                if fo >= end {
                    break;
                }
                if !bits_no_user_data(t, f.ty, start.saturating_sub(fo), end - fo) {
                    return false;
                }
            }
            true
        }
        _ => false,
    }
}

/// Which copy/move special member a constructor is ([class.copy.ctor]/1):
/// `Some(false)` copy, `Some(true)` move, `None` neither.
fn copy_move_ctor_kind(ast: &Ast, ctor: DeclId, class: DeclId) -> Option<bool> {
    use kiln_ast::decl::{DeclKind, ParamData};
    if ast.decls.get(ctor).kind() != DeclKind::CXXConstructor {
        return None;
    }
    let f = ast.decls.function(ctor)?;
    let params: Vec<DeclId> = ast.lists.iter::<DeclId>(f.params).collect();
    let (&first, rest) = params.split_first()?;
    let has_default = |p: DeclId| {
        ast.decls.param(p).is_some_and(|x| {
            x.default_arg.is_some() || x.flags & ParamData::DEFAULT_ARG_PENDING != 0
        })
    };
    if has_default(first) || !rest.iter().all(|&p| has_default(p)) {
        return None;
    }
    let pty = ast.decls.get(first).ty()?;
    let (is_move, inner) = match ast.types.canonical_view(pty) {
        TypeView::LValueRef(t) => (false, t),
        TypeView::RValueRef(t) => (true, t),
        _ => return None,
    };
    let rd = ast.types.as_record(ast.types.canonical(inner))?;
    (ast.first(rd) == ast.first(class)).then_some(is_move)
}

/// clang 19 `canPassInRegisters` (Itanium, C++11 and later): a class
/// whose eligible copy/move constructors and destructor are all trivial
/// or deleted, with at least one non-deleted copy or move constructor,
/// is passed and returned like a C struct; any other class is passed by
/// address and returned through `sret` ([class.temporary]/3).
/// "Trivial" is clang's trivial-for-call: trivial, or made so by
/// `[[clang::trivial_abi]]` on the class or a subobject (the class bits
/// `TRIVIAL_ABI` and `*_FOR_CALL`; spec/abi.md §8.25).
pub fn can_pass_in_registers(ast: &Ast, record: DeclId) -> bool {
    use kiln_ast::decl::{DeclKind, DefinitionBits as B};
    let Some(def) = ast.definition(record) else { return true };
    let Some(r) = ast.decls.record(def) else { return true };
    let bits = r.defn;
    let trivial_abi = bits.contains(B::TRIVIAL_ABI);
    // A declared, non-trivial member is trivial for calls under
    // `trivial_abi`, or when not user-provided and the subobjects' are.
    let member_for_call = |m: DeclId, spec: kiln_ast::decl::FnSpec, b: B| {
        spec.is_trivial() || trivial_abi || (bits.contains(b) && !is_user_provided(ast, m))
    };
    let mut copy_decl = false;
    let mut move_decl = false;
    // Declared constructors (implicit ones included once declared).
    let ctors: Vec<DeclId> = ast.constructors(def).iter().collect();
    let mut declared = Vec::new();
    for c in ctors {
        let Some(kind) = copy_move_ctor_kind(ast, c, def) else { continue };
        let spec = ast.decls.function(c).map(|f| f.spec).unwrap_or_default();
        if kind {
            move_decl = true;
        } else {
            copy_decl = true;
        }
        declared.push((c, spec, if kind { B::MOVE_CTOR_FOR_CALL } else { B::COPY_CTOR_FOR_CALL }));
    }
    let dtor = ast.destructor(def).filter(|&d| ast.decls.get(d).kind() == DeclKind::CXXDestructor);
    let needs_copy = !copy_decl && !bits.contains(B::USER_DECLARED_COPY_CTOR);
    let needs_move = !move_decl
        && !bits.contains(B::USER_DECLARED_MOVE_CTOR)
        && !bits.contains(B::USER_DECLARED_COPY_CTOR)
        && !bits.contains(B::USER_DECLARED_COPY_ASSIGN)
        && !bits.contains(B::USER_DECLARED_MOVE_ASSIGN)
        && !bits.contains(B::USER_DECLARED_DTOR);
    let needs_dtor = dtor.is_none();
    // No "one trivial copy constructor suffices" shortcut: that is the
    // MSVC x64 rule. Under Itanium a non-trivial move constructor (or any
    // non-trivial, non-deleted copy/move constructor or destructor) makes
    // the class indirect even when its copy constructor is trivial.
    let mut has_copy_or_move = false;
    let for_call = |t: B, f: B| bits.contains(t) || bits.contains(f);
    if needs_copy && !bits.contains(B::DEFAULTED_COPY_CTOR_DELETED) {
        if !for_call(B::TRIVIAL_COPY_CTOR, B::COPY_CTOR_FOR_CALL) {
            return false;
        }
        has_copy_or_move = true;
    }
    if needs_move && !bits.contains(B::DEFAULTED_MOVE_CTOR_DELETED) {
        if !for_call(B::TRIVIAL_MOVE_CTOR, B::MOVE_CTOR_FOR_CALL) {
            return false;
        }
        has_copy_or_move = true;
    }
    if needs_dtor
        && !bits.contains(B::DEFAULTED_DTOR_DELETED)
        && !for_call(B::TRIVIAL_DTOR, B::DTOR_FOR_CALL)
    {
        return false;
    }
    for &(c, spec, b) in &declared {
        if spec.is_deleted() {
            continue;
        }
        has_copy_or_move = true;
        if !member_for_call(c, spec, b) {
            return false;
        }
    }
    if let Some(d) = dtor
        && let Some(f) = ast.decls.function(d)
        && !f.spec.is_deleted()
        && !member_for_call(d, f.spec, B::DTOR_FOR_CALL)
    {
        return false;
    }
    has_copy_or_move
}

/// clang `isUserProvided` ([dcl.fct.def.default]/5): user-declared and
/// not defaulted or deleted on its first declaration.
fn is_user_provided(ast: &Ast, f: DeclId) -> bool {
    use kiln_ast::decl::{DeclFlags, Defaulted};
    let first = ast.first(f);
    let spec = ast.decls.function(first).map(|x| x.spec).unwrap_or_default();
    !ast.decls.get(first).flags().contains(DeclFlags::IMPLICIT)
        && spec.defaulted() == Defaulted::No
        && !spec.is_deleted()
}

/// Whether a by-value parameter of class type `q` is destroyed by the
/// callee (clang `isParamDestroyedInCallee`, Itanium: the class is passed
/// in registers yet has a non-trivial destructor, which only
/// `[[clang::trivial_abi]]` on it or a subobject allows; spec/abi.md §8.25).
pub fn is_destroyed_in_callee(ast: &Ast, q: QualType) -> bool {
    use kiln_ast::decl::DefinitionBits as B;
    let TypeView::Record(d) = ast.types.canonical_view(q) else { return false };
    let Some(def) = ast.definition(d) else { return false };
    let Some(r) = ast.decls.record(def) else { return false };
    !r.defn.contains(B::TRIVIAL_DTOR) && can_pass_in_registers(ast, def)
}

/// True if `q` is a class type that [`can_pass_in_registers`] rejects
/// (clang `getRecordArgABI` = `RAA_Indirect`).
pub fn is_indirect_record(ast: &Ast, q: QualType) -> bool {
    match ast.types.canonical_view(q) {
        TypeView::Record(d) => !can_pass_in_registers(ast, d),
        _ => false,
    }
}
