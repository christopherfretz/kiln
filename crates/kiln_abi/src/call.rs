//! Calling-convention lowering (clang `ABIInfo::computeInfo`) for the
//! two supported targets: AAPCS64 (`AArch64ABIInfo`, kind AAPCS) and
//! SysV x86-64 (`X86_64ABIInfo`, no AVX). The result says, for the
//! return value and each argument, how its C++ type is passed in IR.
//! See spec/codegen.md §5.

use kiln_ast::ids::{DeclId, QualType};
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::Ast;

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
/// the first `n_required` are named parameters. `strict_return` is
/// clang's `HasStrictReturn` (false for `extern "C"` callees).
pub fn classify_call(
    t: &mut IrTypes<'_>,
    ret: QualType,
    args: &[QualType],
    n_required: usize,
    strict_return: bool,
) -> CallInfo {
    let mut info = match t.target().arch() {
        Arch::AArch64 => aarch64(t, ret, args),
        Arch::X86_64 => x86_64(t, ret, args, n_required),
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

fn vector_width(t: &mut IrTypes<'_>, ty: &IrType) -> u64 {
    match ty {
        IrType::Vector(..) => t.size_align(ty).0 * 8,
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

fn view(t: &IrTypes<'_>, q: QualType) -> TypeView {
    t.ast().types.canonical_view(q)
}

fn type_bits(t: &mut IrTypes<'_>, q: QualType) -> u64 {
    t.layouter().type_layout(q).size * 8
}

fn type_align_bits(t: &mut IrTypes<'_>, q: QualType) -> u64 {
    t.layouter().type_layout(q).align * 8
}

/// clang `isAggregateTypeForABI`.
fn is_aggregate(t: &IrTypes<'_>, q: QualType) -> bool {
    match view(t, q) {
        TypeView::Record(_) | TypeView::ConstantArray(_) | TypeView::IncompleteArray(_) | TypeView::Complex(_) => true,
        TypeView::MemberPointer { pointee, .. } => {
            matches!(t.ast().types.canonical_view(pointee), TypeView::Function(_))
        }
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
        _ => t.size_align(ty).0 * 8,
    }
}

// ---------------------------------------------------------------- AAPCS64

fn aarch64(t: &mut IrTypes<'_>, ret: QualType, args: &[QualType]) -> CallInfo {
    let r = aarch64_ret(t, ret);
    let a = args.iter().map(|q| arg(aarch64_arg(t, *q))).collect();
    CallInfo { ret: arg(r), args: a, max_vector_width: 0 }
}

fn aarch64_arg(t: &mut IrTypes<'_>, q: QualType) -> ArgKind {
    if !is_aggregate(t, q) {
        let q = strip_enum(t, q);
        if let TypeView::BitInt { width, .. } = view(t, q) {
            if width > 128 {
                return ArgKind::Indirect { byval: false, align: t.layouter().type_layout(q).align };
            }
        }
        return direct(t.convert(q));
    }
    let size = type_bits(t, q);
    let empty = is_empty_record(t, q, true);
    if empty || size == 0 {
        // C++, non-Darwin.
        if empty && size == 0 {
            return ArgKind::Ignore;
        }
        return direct(IrType::Int(8));
    }
    if let Some((base, members)) = homogeneous(t, q) {
        let align = t.layouter().type_layout(q).align;
        let bt = t.convert(base);
        return ArgKind::Direct {
            coerce: IrType::Array(Box::new(bt), members),
            offset: 0,
            flatten: true,
            align: Some(if align >= 16 { 16 } else { 8 }),
        };
    }
    if size <= 128 {
        let a = type_align_bits(t, q);
        let a = if a < 128 { 64 } else { 128 };
        let size = size.next_multiple_of(a);
        let base = IrType::Int(u32::try_from(a).unwrap_or(64));
        return direct(if size == a { base } else { IrType::Array(Box::new(base), size / a) });
    }
    ArgKind::Indirect { byval: false, align: t.layouter().type_layout(q).align }
}

fn aarch64_ret(t: &mut IrTypes<'_>, q: QualType) -> ArgKind {
    if matches!(view(t, q), TypeView::Builtin(BuiltinType::Void)) {
        return ArgKind::Ignore;
    }
    if !is_aggregate(t, q) {
        let q = strip_enum(t, q);
        if let TypeView::BitInt { width, .. } = view(t, q) {
            if width > 128 {
                return ArgKind::Indirect { byval: true, align: t.layouter().type_layout(q).align };
            }
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

/// clang `isHomogeneousAggregate` with AArch64's base-type and size rules.
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
    use Class::*;
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
    use Class::*;
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
    use Class::*;
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
            BuiltinType::Float | BuiltinType::Double | BuiltinType::Float16 | BuiltinType::BFloat16 => {
                set_cur!(Sse)
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
        TypeView::Pointer(_) | TypeView::LValueRef(_) | TypeView::RValueRef(_) | TypeView::Function(_) => {
            set_cur!(Integer)
        }
        TypeView::MemberPointer { pointee, .. } => {
            if matches!(ast.types.canonical_view(pointee), TypeView::Function(_)) {
                lo = Integer;
                hi = Integer;
            } else {
                set_cur!(Integer)
            }
        }
        TypeView::BitInt { width, .. } => {
            if width <= 64 {
                set_cur!(Integer)
            } else if width <= 128 {
                lo = Integer;
                hi = Integer;
            }
        }
        TypeView::Complex(e) => {
            let size = type_bits(t, q);
            let esize = type_bits(t, e);
            match view(t, e) {
                TypeView::Builtin(BuiltinType::Float | BuiltinType::Float16 | BuiltinType::BFloat16) => {
                    set_cur!(Sse)
                }
                TypeView::Builtin(BuiltinType::Double) => {
                    lo = Sse;
                    hi = Sse;
                }
                TypeView::Builtin(BuiltinType::LongDouble) => set_cur!(ComplexX87),
                _ => {
                    if size <= 64 {
                        set_cur!(Integer)
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
        TypeView::ConstantArray(a) => {
            let size = type_bits(t, q);
            if size > 512 {
                return (lo, hi);
            }
            if off % type_align_bits(t, a.elem) != 0 {
                return (lo, hi);
            }
            set_cur!(NoClass);
            let esize = type_bits(t, a.elem);
            let n = ast.types.array_size(q).unwrap_or(0);
            if size > 128 && (size != esize || size > 128) {
                return (lo, hi);
            }
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
            let layout = t.layouter().record(def).clone();
            set_cur!(NoClass);
            let is_union = props::class_desc(ast, def).is_union;
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
                if size > 128 && ((!is_union && size != type_bits(t, f.ty)) || size > 128) {
                    lo = Memory;
                    post_merge(size, &mut lo, &mut hi);
                    return (lo, hi);
                }
                let in_memory = foff % type_align_bits(t, f.ty) != 0;
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

fn x86_64(t: &mut IrTypes<'_>, ret: QualType, args: &[QualType], _n_required: usize) -> CallInfo {
    let mut free_int = 6u32;
    let mut free_sse = 8u32;
    let r = x86_ret(t, ret);
    if matches!(r, ArgKind::Indirect { .. }) {
        free_int -= 1;
    }
    let mut out = Vec::with_capacity(args.len());
    for &q in args {
        let (mut k, ni, ns) = x86_arg(t, q, free_int);
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

fn x86_indirect(t: &mut IrTypes<'_>, q: QualType, free_int: u32) -> ArgKind {
    if !is_aggregate(t, q) && !matches!(view(t, q), TypeView::BitInt { .. }) {
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
    use Class::*;
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
        Memory | X87 | ComplexX87 => return (x86_indirect(t, q, free_int), 0, 0),
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
                return (ArgKind::Direct { coerce: h, offset: 8, flatten: true, align: None }, ni, ns);
            }
            high = Some(h);
        }
        X87Up | Sse => {
            ns += 1;
            let h = sse_at(t, &natural, 8, q, 8);
            if lo == NoClass {
                return (ArgKind::Direct { coerce: h, offset: 8, flatten: true, align: None }, ni, ns);
            }
            high = Some(h);
        }
        SseUp => res = Some(natural.clone()),
        _ => {}
    }
    let res = match (res, high) {
        (Some(l), Some(h)) => pair(t, l, h),
        (Some(l), None) => l,
        (None, _) => natural,
    };
    (direct(res), ni, ns)
}

fn x86_ret(t: &mut IrTypes<'_>, q: QualType) -> ArgKind {
    use Class::*;
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
            if !is_aggregate(t, q) {
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
        SseUp => res = Some(natural.clone()),
        X87Up => {
            if lo != X87 {
                let h = sse_at(t, &natural, 8, q, 8);
                if lo == NoClass {
                    return ArgKind::Direct { coerce: h, offset: 8, flatten: true, align: None };
                }
                high = Some(h);
            }
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
    let lo = if hi_start != 8 {
        if matches!(lo, IrType::Half | IrType::Float) { IrType::Double } else { IrType::Int(64) }
    } else {
        lo
    };
    IrType::Struct(vec![lo, hi], false)
}

/// The struct element containing byte `off` and the offset within it.
fn element_at(t: &mut IrTypes<'_>, members: &[IrType], packed: bool, off: u64) -> Option<(IrType, u64)> {
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
        _ => None,
    }
}

/// clang `GetINTEGERTypeAtOffset`.
fn integer_at(t: &mut IrTypes<'_>, ir: &IrType, ir_off: u64, src: QualType, src_off: u64) -> IrType {
    if ir_off == 0 {
        if matches!(ir, IrType::Ptr | IrType::Int(64)) {
            return ir.clone();
        }
        if let IrType::Int(w @ (8 | 16 | 32)) = ir {
            if bits_no_user_data(t, src, src_off * 8 + u64::from(*w), src_off * 8 + 64) {
                return ir.clone();
            }
        }
    }
    if let Some((m, p)) = struct_body(t, ir) {
        let (size, _) = t.size_align(ir);
        if ir_off < size {
            if let Some((e, o)) = element_at(t, &m, p, ir_off) {
                return integer_at(t, &e, o, src, src_off);
            }
        }
    }
    if let IrType::Array(e, _) = ir {
        let (es, _) = t.size_align(e);
        let eo = if es == 0 { 0 } else { ir_off / es * es };
        return integer_at(t, e, ir_off - eo, src, src_off);
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
        let o = if es == 0 { 0 } else { off - off / es * es };
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
