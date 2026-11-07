//! Declarations of LLVM intrinsics (`spec/llvm.md` §6).
//!
//! An intrinsic is an ordinary function declaration whose name starts with
//! `llvm.`. These helpers build the overloaded name, the function type, and
//! the declaration attributes LLVM 19 attaches (so the printed declaration
//! matches clang's), and return the existing declaration when the module
//! already has one.

use crate::attr::{Attr, AttrKind, MemoryEffects, ModRef};
use crate::error::IrResult;
use crate::module::Module;
use crate::types::TypeId;
use crate::value::FuncId;

use AttrKind::{
    Cold, ImmArg, NoAlias, NoCallback, NoCapture, NoFree, NoReturn, NoSync, NoUndef, NoUnwind,
    NonNull, ReadOnly, Speculatable, WillReturn, WriteOnly,
};

/// Function-attribute shapes used by the intrinsics below.
#[derive(Copy, Clone)]
enum Shape {
    /// `nocallback nofree nounwind willreturn memory(argmem: ...)`.
    ArgMem(ModRef),
    /// `nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)`.
    NoSyncArgMem,
    /// `nocallback nofree nosync nounwind speculatable willreturn memory(none)`.
    Pure,
    /// `nocallback nofree nosync nounwind willreturn [memory(...)]`.
    Basic(Option<MemoryEffects>),
    /// `cold noreturn nounwind memory(inaccessiblemem: write)`.
    Trap,
    /// `nounwind [memory(none)]`.
    NoUnwind(bool),
    /// `nounwind memory(argmem: read)`.
    NoUnwindArgRead,
    /// `nounwind memory(...)`.
    NoUnwindMem(MemoryEffects),
    /// `nomerge nounwind`.
    NoMerge,
    /// No attributes (an intrinsic that may throw).
    Throws,
}

fn fn_attrs(shape: Shape) -> Vec<Attr> {
    let f = Attr::Flag;
    match shape {
        Shape::ArgMem(m) => vec![
            f(NoCallback),
            f(NoFree),
            f(NoUnwind),
            f(WillReturn),
            Attr::Memory(MemoryEffects::argmem_only(m)),
        ],
        Shape::NoSyncArgMem => vec![
            f(NoCallback),
            f(NoFree),
            f(NoSync),
            f(NoUnwind),
            f(WillReturn),
            Attr::Memory(MemoryEffects::argmem_only(ModRef::ReadWrite)),
        ],
        Shape::Pure => vec![
            f(NoCallback),
            f(NoFree),
            f(NoSync),
            f(NoUnwind),
            f(Speculatable),
            f(WillReturn),
            Attr::Memory(MemoryEffects::all(ModRef::None)),
        ],
        Shape::Basic(mem) => {
            let mut v = vec![f(NoCallback), f(NoFree), f(NoSync), f(NoUnwind), f(WillReturn)];
            v.extend(mem.map(Attr::Memory));
            v
        }
        Shape::Trap => vec![
            f(Cold),
            f(NoReturn),
            f(NoUnwind),
            Attr::Memory(MemoryEffects {
                inaccessiblemem: ModRef::Write,
                ..MemoryEffects::default()
            }),
        ],
        Shape::NoUnwind(none) => {
            let mut v = vec![f(NoUnwind)];
            if none {
                v.push(Attr::Memory(MemoryEffects::all(ModRef::None)));
            }
            v
        }
        Shape::NoUnwindArgRead => {
            vec![f(NoUnwind), Attr::Memory(MemoryEffects::argmem_only(ModRef::Read))]
        }
        Shape::NoUnwindMem(mem) => vec![f(NoUnwind), Attr::Memory(mem)],
        Shape::NoMerge => vec![f(AttrKind::NoMerge), f(NoUnwind)],
        Shape::Throws => Vec::new(),
    }
}

fn inaccessible_write() -> MemoryEffects {
    MemoryEffects { inaccessiblemem: ModRef::Write, ..MemoryEffects::default() }
}

/// The overloaded intrinsic name: `base` followed by `.suffix` for each
/// overload type (`llvm.memcpy` + `[ptr, ptr, i64]` → `llvm.memcpy.p0.p0.i64`).
pub fn overloaded_name(m: &Module, base: &str, overloads: &[TypeId]) -> String {
    let mut s = base.to_owned();
    for &t in overloads {
        s.push('.');
        s.push_str(&m.types.intrinsic_suffix(t));
    }
    s
}

/// Declare (or find) an intrinsic by full name and function type, without
/// attributes. Use for intrinsics that have no dedicated helper. LLVM's
/// parser attaches an intrinsic's built-in attributes when it reads the
/// declaration, so set `attrs.func` to match them if the printed text is to
/// be canonical (`spec/llvm.md` §6).
///
/// # Errors
///
/// Errors unless `name` starts with `llvm.` and `fn_ty` is a function type
/// (or if a function of that name exists with another type).
pub fn intrinsic(m: &mut Module, name: &str, fn_ty: TypeId) -> IrResult<FuncId> {
    if !name.starts_with("llvm.") {
        return Err(crate::IrError::new(
            crate::IrErrorKind::BadFunction,
            format!("intrinsic name {name:?} does not start with \"llvm.\""),
        ));
    }
    m.get_or_insert_function(name, fn_ty)
}

/// The intrinsics LLVM 22 marks `IntrNoCreateUndefOrPoison` (its result is
/// neither undef nor poison when no argument is), by base name. LLVM 22's
/// parser attaches `nocreateundeforpoison` to their declarations; LLVM 19
/// has no such attribute (`spec/llvm.md` §6).
const NO_CREATE_UNDEF_OR_POISON: &[&str] = &[
    "llvm.arithmetic.fence",
    "llvm.bitreverse",
    "llvm.bswap",
    "llvm.canonicalize",
    "llvm.ceil",
    "llvm.clmul",
    "llvm.copysign",
    "llvm.cos",
    "llvm.ctpop",
    "llvm.exp",
    "llvm.exp10",
    "llvm.exp2",
    "llvm.fabs",
    "llvm.floor",
    "llvm.fma",
    "llvm.fmuladd",
    "llvm.fptosi.sat",
    "llvm.fptoui.sat",
    "llvm.fptrunc.round",
    "llvm.frexp",
    "llvm.fshl",
    "llvm.fshr",
    "llvm.is.fpclass",
    "llvm.ldexp",
    "llvm.llrint",
    "llvm.llround",
    "llvm.log",
    "llvm.log10",
    "llvm.log2",
    "llvm.lrint",
    "llvm.lround",
    "llvm.maximum",
    "llvm.maximumnum",
    "llvm.maxnum",
    "llvm.minimum",
    "llvm.minimumnum",
    "llvm.minnum",
    "llvm.nearbyint",
    "llvm.pow",
    "llvm.powi",
    "llvm.ptrmask",
    "llvm.rint",
    "llvm.round",
    "llvm.roundeven",
    "llvm.sadd.sat",
    "llvm.sadd.with.overflow",
    "llvm.scmp",
    "llvm.sin",
    "llvm.smax",
    "llvm.smin",
    "llvm.smul.with.overflow",
    "llvm.sqrt",
    "llvm.ssub.sat",
    "llvm.ssub.with.overflow",
    "llvm.trunc",
    "llvm.uadd.sat",
    "llvm.uadd.with.overflow",
    "llvm.ucmp",
    "llvm.umax",
    "llvm.umin",
    "llvm.umul.with.overflow",
    "llvm.usub.sat",
    "llvm.usub.with.overflow",
];

/// Whether the declaration of intrinsic `name` (overload suffixes
/// included) carries `nocreateundeforpoison`: at [`Module::ir_version`] 22
/// or later, for the intrinsics LLVM 22 marks (`abs`, `ctlz`, `cttz` and
/// the vector reductions are not among them).
pub fn creates_no_undef_or_poison(m: &Module, name: &str) -> bool {
    m.ir_version >= 22
        && NO_CREATE_UNDEF_OR_POISON.iter().any(|b| {
            name.strip_prefix(b).is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
        })
}

fn declare(
    m: &mut Module,
    name: &str,
    ret: TypeId,
    params: &[(TypeId, &[Attr])],
    ret_attrs: &[Attr],
    shape: Shape,
) -> FuncId {
    if let Some(f) = m.get_function(name) {
        return f;
    }
    let tys: Vec<TypeId> = params.iter().map(|p| p.0).collect();
    let fn_ty = m.types.function(ret, &tys, false);
    let f = m.add_function(name, fn_ty).expect("intrinsic type is a function type");
    let mut fattrs = fn_attrs(shape);
    if creates_no_undef_or_poison(m, name) {
        fattrs.push(Attr::Flag(AttrKind::NoCreateUndefOrPoison));
    }
    let fa = m.attr_set(&fattrs);
    let ra = m.attr_set(ret_attrs);
    let pa: Vec<_> = params.iter().map(|p| m.attr_set(p.1)).collect();
    let func = m.func_mut(f);
    func.attrs.func = fa;
    func.attrs.ret = ra;
    for (i, a) in pa.into_iter().enumerate() {
        func.attrs.set_param(i, a);
    }
    f
}

fn ptr(m: &mut Module) -> TypeId {
    m.types.ptr(0)
}

fn mem_transfer(m: &mut Module, base: &str, len: TypeId, noalias: bool) -> FuncId {
    let p = ptr(m);
    let name = overloaded_name(m, base, &[p, p, len]);
    let f = Attr::Flag;
    let (dst, src): (&[Attr], &[Attr]) = if noalias {
        (&[f(NoAlias), f(NoCapture), f(WriteOnly)], &[f(NoAlias), f(NoCapture), f(ReadOnly)])
    } else {
        (&[f(NoCapture), f(WriteOnly)], &[f(NoCapture), f(ReadOnly)])
    };
    declare(
        m,
        &name,
        TypeId::VOID,
        &[(p, dst), (p, src), (len, &[]), (TypeId::I1, &[f(ImmArg)])],
        &[],
        Shape::ArgMem(ModRef::ReadWrite),
    )
}

/// `void @llvm.memcpy.p0.p0.iN(ptr dst, ptr src, iN len, i1 volatile)`.
pub fn memcpy(m: &mut Module, len: TypeId) -> FuncId {
    mem_transfer(m, "llvm.memcpy", len, true)
}

/// `void @llvm.memcpy.inline.p0.p0.iN(ptr dst, ptr src, iN len, i1
/// volatile)`: a `memcpy` never lowered to a library call.
pub fn memcpy_inline(m: &mut Module, len: TypeId) -> FuncId {
    mem_transfer(m, "llvm.memcpy.inline", len, true)
}

/// `void @llvm.memmove.p0.p0.iN(ptr dst, ptr src, iN len, i1 volatile)`.
pub fn memmove(m: &mut Module, len: TypeId) -> FuncId {
    mem_transfer(m, "llvm.memmove", len, false)
}

/// `void @llvm.memset.p0.iN(ptr dst, i8 val, iN len, i1 volatile)`.
pub fn memset(m: &mut Module, len: TypeId) -> FuncId {
    let p = ptr(m);
    let name = overloaded_name(m, "llvm.memset", &[p, len]);
    let f = Attr::Flag;
    declare(
        m,
        &name,
        TypeId::VOID,
        &[
            (p, &[f(NoCapture), f(WriteOnly)]),
            (TypeId::I8, &[]),
            (len, &[]),
            (TypeId::I1, &[f(ImmArg)]),
        ],
        &[],
        Shape::ArgMem(ModRef::Write),
    )
}

fn lifetime(m: &mut Module, base: &str) -> FuncId {
    let p = ptr(m);
    let name = overloaded_name(m, base, &[p]);
    let sized: &[(TypeId, &[Attr])] =
        &[(TypeId::I64, &[Attr::Flag(ImmArg)]), (p, &[Attr::Flag(NoCapture)])];
    let sizeless: &[(TypeId, &[Attr])] = &[(p, &[Attr::Flag(NoCapture)])];
    let params = if lifetime_has_size(m) { sized } else { sizeless };
    declare(m, &name, TypeId::VOID, params, &[], Shape::NoSyncArgMem)
}

/// Whether `llvm.lifetime.start`/`.end` take the object size first: below
/// [`Module::ir_version`] 22 (LLVM 22 dropped the operand; LLVM 21 keeps
/// it but its verifier rejects the sizeless form, and from 21 on the
/// `ptr` is written `captures(none)`).
pub fn lifetime_has_size(m: &Module) -> bool {
    m.ir_version < 22
}

/// `void @llvm.lifetime.start.p0(i64 immarg size, ptr nocapture)` (at
/// [`Module::ir_version`] 21 `captures(none)`); at 22+ `void
/// @llvm.lifetime.start.p0(ptr captures(none))`.
pub fn lifetime_start(m: &mut Module) -> FuncId {
    lifetime(m, "llvm.lifetime.start")
}

/// `void @llvm.lifetime.end.p0(i64 immarg size, ptr nocapture)`; at
/// [`Module::ir_version`] 22+ without the size.
pub fn lifetime_end(m: &mut Module) -> FuncId {
    lifetime(m, "llvm.lifetime.end")
}

/// `ptr @llvm.invariant.start.p0(i64 immarg size, ptr nocapture)`.
pub fn invariant_start(m: &mut Module) -> FuncId {
    let p = ptr(m);
    let name = overloaded_name(m, "llvm.invariant.start", &[p]);
    declare(
        m,
        &name,
        p,
        &[(TypeId::I64, &[Attr::Flag(ImmArg)]), (p, &[Attr::Flag(NoCapture)])],
        &[],
        Shape::NoSyncArgMem,
    )
}

/// `void @llvm.trap()`.
pub fn trap(m: &mut Module) -> FuncId {
    declare(m, "llvm.trap", TypeId::VOID, &[], &[], Shape::Trap)
}

/// `void @llvm.debugtrap()`.
pub fn debugtrap(m: &mut Module) -> FuncId {
    declare(m, "llvm.debugtrap", TypeId::VOID, &[], &[], Shape::NoUnwind(false))
}

/// `void @llvm.prefetch.p0(ptr nocapture readonly, i32 immarg rw, i32
/// immarg locality, i32 immarg cache)`: `nocallback nofree nosync nounwind
/// willreturn memory(argmem: readwrite, inaccessiblemem: readwrite)`
/// (WP-610).
pub fn prefetch(m: &mut Module) -> FuncId {
    let p = ptr(m);
    let name = overloaded_name(m, "llvm.prefetch", &[p]);
    let f = Attr::Flag;
    let imm: &[Attr] = &[f(ImmArg)];
    let params: [(TypeId, &[Attr]); 4] = [
        (p, &[f(NoCapture), f(ReadOnly)]),
        (TypeId::I32, imm),
        (TypeId::I32, imm),
        (TypeId::I32, imm),
    ];
    let mem = MemoryEffects {
        argmem: ModRef::ReadWrite,
        inaccessiblemem: ModRef::ReadWrite,
        ..MemoryEffects::default()
    };
    declare(m, &name, TypeId::VOID, &params, &[], Shape::Basic(Some(mem)))
}

/// `ptr @llvm.frameaddress.p0(i32 immarg level)`.
pub fn frameaddress(m: &mut Module) -> FuncId {
    let p = ptr(m);
    let name = overloaded_name(m, "llvm.frameaddress", &[p]);
    let none = MemoryEffects::all(ModRef::None);
    let params: [(TypeId, &[Attr]); 1] = [(TypeId::I32, &[Attr::Flag(ImmArg)])];
    declare(m, &name, p, &params, &[], Shape::Basic(Some(none)))
}

/// A target intrinsic without memory effects, e.g.
/// `<4 x float> @llvm.aarch64.neon.bfdot.v4f32.v8bf16(...)`:
/// `nocallback nofree nosync nounwind willreturn memory(none)` (LLVM's
/// `IntrNoMem` target intrinsics, which are not `speculatable`; WP-558).
/// `name` is the full (overloaded) name.
pub fn target_nomem(m: &mut Module, name: &str, ret: TypeId, params: &[TypeId]) -> FuncId {
    let params: Vec<(TypeId, &[Attr])> = params.iter().map(|&t| (t, &[][..])).collect();
    declare(m, name, ret, &params, &[], Shape::Basic(Some(MemoryEffects::all(ModRef::None))))
}

/// The attributes of a target intrinsic, as LLVM declares the `llvm.x86.*`
/// ones (its `IntrNoMem`, `IntrReadMem`, `IntrArgMemOnly`, ... properties;
/// the variants are the shapes the x86 builtin table uses, WP-630).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TargetMem {
    /// `nocallback nofree nosync nounwind willreturn memory(none)`.
    None,
    /// `nocallback nofree nosync nounwind willreturn memory(read)`.
    Read,
    /// `nocallback nofree nosync nounwind willreturn memory(argmem: read)`.
    ArgRead,
    /// `nocallback nofree nosync nounwind willreturn memory(argmem:
    /// readwrite)`.
    ArgReadWrite,
    /// `nounwind` (any memory effect).
    Unknown,
    /// `nounwind memory(argmem: readwrite)`.
    UnknownArgReadWrite,
    /// `nounwind memory(argmem: write)`.
    UnknownArgWrite,
    /// `nounwind memory(inaccessiblemem: readwrite)`.
    UnknownInaccessible,
}

/// A target intrinsic by full name, return type and parameter types (each
/// with whether it is `immarg`), with the attributes `mem` names
/// (WP-630's x86 builtins).
pub fn target_intrinsic(
    m: &mut Module,
    name: &str,
    ret: TypeId,
    params: &[(TypeId, bool)],
    mem: TargetMem,
) -> FuncId {
    let shape = match mem {
        TargetMem::None => Shape::Basic(Some(MemoryEffects::all(ModRef::None))),
        TargetMem::Read => Shape::Basic(Some(MemoryEffects::all(ModRef::Read))),
        TargetMem::ArgRead => Shape::Basic(Some(MemoryEffects::argmem_only(ModRef::Read))),
        TargetMem::ArgReadWrite => {
            Shape::Basic(Some(MemoryEffects::argmem_only(ModRef::ReadWrite)))
        }
        TargetMem::Unknown => Shape::NoUnwind(false),
        TargetMem::UnknownArgReadWrite => {
            Shape::NoUnwindMem(MemoryEffects::argmem_only(ModRef::ReadWrite))
        }
        TargetMem::UnknownArgWrite => Shape::NoUnwindMem(MemoryEffects::argmem_only(ModRef::Write)),
        TargetMem::UnknownInaccessible => Shape::NoUnwindMem(MemoryEffects {
            inaccessiblemem: ModRef::ReadWrite,
            ..MemoryEffects::default()
        }),
    };
    let imm: &[Attr] = &[Attr::Flag(ImmArg)];
    let params: Vec<(TypeId, &[Attr])> =
        params.iter().map(|&(t, i)| (t, if i { imm } else { &[][..] })).collect();
    declare(m, name, ret, &params, &[], shape)
}

/// `void @llvm.aarch64.neon.st{2,3,4}.<VTy>.p0(<VTy> x k, ptr nocapture)`
/// by full name, as LLVM declares it (`IntrArgMemOnly`,
/// `NoCapture<ArgIndex<k>>`; WP-799).
pub fn neon_store_n(m: &mut Module, name: &str, vt: TypeId, k: usize) -> FuncId {
    let p = ptr(m);
    let nc: &[Attr] = &[Attr::Flag(NoCapture)];
    let mut params: Vec<(TypeId, &[Attr])> = vec![(vt, &[][..]); k];
    params.push((p, nc));
    let mem = MemoryEffects::argmem_only(ModRef::ReadWrite);
    declare(m, name, TypeId::VOID, &params, &[], Shape::Basic(Some(mem)))
}

/// `ptr @llvm.returnaddress(i32 immarg level)` (WP-432).
pub fn returnaddress(m: &mut Module) -> FuncId {
    let p = ptr(m);
    let none = MemoryEffects::all(ModRef::None);
    let params: [(TypeId, &[Attr]); 1] = [(TypeId::I32, &[Attr::Flag(ImmArg)])];
    declare(m, "llvm.returnaddress", p, &params, &[], Shape::Basic(Some(none)))
}

/// Arithmetic of an `llvm.*.with.overflow.*` intrinsic.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum OverflowOp {
    /// Signed add.
    SAdd,
    /// Unsigned add.
    UAdd,
    /// Signed subtract.
    SSub,
    /// Unsigned subtract.
    USub,
    /// Signed multiply.
    SMul,
    /// Unsigned multiply.
    UMul,
}

/// `{ T, i1 } @llvm.<op>.with.overflow.T(T, T)` for an integer (vector) `T`.
pub fn with_overflow(m: &mut Module, op: OverflowOp, ty: TypeId) -> FuncId {
    let base = match op {
        OverflowOp::SAdd => "llvm.sadd.with.overflow",
        OverflowOp::UAdd => "llvm.uadd.with.overflow",
        OverflowOp::SSub => "llvm.ssub.with.overflow",
        OverflowOp::USub => "llvm.usub.with.overflow",
        OverflowOp::SMul => "llvm.smul.with.overflow",
        OverflowOp::UMul => "llvm.umul.with.overflow",
    };
    let name = overloaded_name(m, base, &[ty]);
    let flag = match m.types.vector_info(ty) {
        Some((_, n, scalable)) if scalable => m.types.scalable_vector(TypeId::I1, n),
        Some((_, n, _)) => m.types.vector(TypeId::I1, n),
        None => TypeId::I1,
    };
    let ret = m.types.struct_literal(&[ty, flag], false);
    declare(m, &name, ret, &[(ty, &[]), (ty, &[])], &[], Shape::Pure)
}

fn va(m: &mut Module, base: &str, n: usize) -> FuncId {
    let p = ptr(m);
    let name = overloaded_name(m, base, &[p]);
    let params: Vec<(TypeId, &[Attr])> = vec![(p, &[]); n];
    declare(m, &name, TypeId::VOID, &params, &[], Shape::Basic(None))
}

/// `void @llvm.va_start.p0(ptr)`.
pub fn va_start(m: &mut Module) -> FuncId {
    va(m, "llvm.va_start", 1)
}

/// `void @llvm.va_end.p0(ptr)`.
pub fn va_end(m: &mut Module) -> FuncId {
    va(m, "llvm.va_end", 1)
}

/// `void @llvm.va_copy.p0(ptr dst, ptr src)`.
pub fn va_copy(m: &mut Module) -> FuncId {
    va(m, "llvm.va_copy", 2)
}

/// `ptr @llvm.ptrmask.p0.i64(ptr, i64)`.
pub fn ptrmask(m: &mut Module) -> FuncId {
    let p = ptr(m);
    let name = overloaded_name(m, "llvm.ptrmask", &[p, TypeId::I64]);
    declare(m, &name, p, &[(p, &[]), (TypeId::I64, &[])], &[], Shape::Pure)
}

/// `ptr @llvm.stacksave.p0()`.
pub fn stacksave(m: &mut Module) -> FuncId {
    let p = ptr(m);
    let name = overloaded_name(m, "llvm.stacksave", &[p]);
    declare(m, &name, p, &[], &[], Shape::Basic(None))
}

/// `void @llvm.stackrestore.p0(ptr)`.
pub fn stackrestore(m: &mut Module) -> FuncId {
    va(m, "llvm.stackrestore", 1)
}

/// `nonnull ptr @llvm.threadlocal.address.p0(ptr nonnull)`.
pub fn threadlocal_address(m: &mut Module) -> FuncId {
    let p = ptr(m);
    let name = overloaded_name(m, "llvm.threadlocal.address", &[p]);
    let nn = [Attr::Flag(NonNull)];
    declare(m, &name, p, &[(p, &nn)], &nn, Shape::Pure)
}

/// `i32 @llvm.eh.typeid.for.p0(ptr)`.
pub fn eh_typeid_for(m: &mut Module) -> FuncId {
    let p = ptr(m);
    let name = overloaded_name(m, "llvm.eh.typeid.for", &[p]);
    declare(m, &name, TypeId::I32, &[(p, &[])], &[], Shape::NoUnwind(true))
}

/// `T @llvm.expect.T(T, T)`.
pub fn expect(m: &mut Module, ty: TypeId) -> FuncId {
    let name = overloaded_name(m, "llvm.expect", &[ty]);
    let none = MemoryEffects::all(ModRef::None);
    declare(m, &name, ty, &[(ty, &[]), (ty, &[])], &[], Shape::Basic(Some(none)))
}

/// `T @llvm.expect.with.probability.T(T, T, double immarg)`.
pub fn expect_with_probability(m: &mut Module, ty: TypeId) -> FuncId {
    let name = overloaded_name(m, "llvm.expect.with.probability", &[ty]);
    let none = MemoryEffects::all(ModRef::None);
    let params: &[(TypeId, &[Attr])] =
        &[(ty, &[]), (ty, &[]), (TypeId::DOUBLE, &[Attr::Flag(ImmArg)])];
    declare(m, &name, ty, params, &[], Shape::Basic(Some(none)))
}

/// `void @llvm.assume(i1 noundef)`.
pub fn assume(m: &mut Module) -> FuncId {
    let shape = Shape::Basic(Some(inaccessible_write()));
    declare(m, "llvm.assume", TypeId::VOID, &[(TypeId::I1, &[Attr::Flag(NoUndef)])], &[], shape)
}

fn unary_flag(m: &mut Module, base: &str, ty: TypeId) -> FuncId {
    let name = overloaded_name(m, base, &[ty]);
    declare(m, &name, ty, &[(ty, &[]), (TypeId::I1, &[Attr::Flag(ImmArg)])], &[], Shape::Pure)
}

fn unary(m: &mut Module, base: &str, ty: TypeId) -> FuncId {
    let name = overloaded_name(m, base, &[ty]);
    declare(m, &name, ty, &[(ty, &[])], &[], Shape::Pure)
}

/// `E @llvm.vector.reduce.<op>.V(V)` for a vector type `V` of element
/// type `E` (`op`: `add`, `mul`, `and`, `or`, `xor`, `smax`, `smin`,
/// `umax`, `umin`, `fmax`, `fmin`).
pub fn vector_reduce(m: &mut Module, op: &str, vty: TypeId) -> FuncId {
    let et = m.types.vector_info(vty).map_or(vty, |x| x.0);
    let name = overloaded_name(m, &format!("llvm.vector.reduce.{op}"), &[vty]);
    declare(m, &name, et, &[(vty, &[])], &[], Shape::Pure)
}

/// `T @llvm.abs.T(T, i1 immarg is_int_min_poison)`.
pub fn abs(m: &mut Module, ty: TypeId) -> FuncId {
    unary_flag(m, "llvm.abs", ty)
}

/// `T @llvm.ctlz.T(T, i1 immarg is_zero_poison)`.
pub fn ctlz(m: &mut Module, ty: TypeId) -> FuncId {
    unary_flag(m, "llvm.ctlz", ty)
}

/// `T @llvm.cttz.T(T, i1 immarg is_zero_poison)`.
pub fn cttz(m: &mut Module, ty: TypeId) -> FuncId {
    unary_flag(m, "llvm.cttz", ty)
}

/// `T @llvm.ctpop.T(T)`.
pub fn ctpop(m: &mut Module, ty: TypeId) -> FuncId {
    unary(m, "llvm.ctpop", ty)
}

/// `T @llvm.bswap.T(T)`.
pub fn bswap(m: &mut Module, ty: TypeId) -> FuncId {
    unary(m, "llvm.bswap", ty)
}

/// `T @llvm.fabs.T(T)`.
pub fn fabs(m: &mut Module, ty: TypeId) -> FuncId {
    unary(m, "llvm.fabs", ty)
}

/// `T @llvm.<op>.T(T, ...)` with `arity` operands of floating-point type
/// `T`: the elementwise floating-point intrinsics (`llvm.copysign`,
/// `llvm.maxnum`, `llvm.ceil`, `llvm.fma`, ...). `base` is the full
/// unmangled name, e.g. `"llvm.copysign"`.
pub fn fp_op(m: &mut Module, base: &str, ty: TypeId, arity: usize) -> FuncId {
    let name = overloaded_name(m, base, &[ty]);
    let params: Vec<(TypeId, &[Attr])> = (0..arity).map(|_| (ty, &[][..])).collect();
    declare(m, &name, ty, &params, &[], Shape::Pure)
}

/// `T @llvm.powi.T.I(T, I)`: a floating-point `T` raised to an integer
/// power of type `exp` (WP-282, `__builtin_powi`).
pub fn powi(m: &mut Module, ty: TypeId, exp: TypeId) -> FuncId {
    let name = overloaded_name(m, "llvm.powi", &[ty, exp]);
    declare(m, &name, ty, &[(ty, &[]), (exp, &[])], &[], Shape::Pure)
}

/// `R @llvm.objectsize.R.p0(ptr, i1 immarg min, i1 immarg nullunknown,
/// i1 immarg dynamic)` for an integer result `R` (WP-282,
/// `__builtin_object_size`).
pub fn objectsize(m: &mut Module, ret: TypeId) -> FuncId {
    let p = ptr(m);
    let name = overloaded_name(m, "llvm.objectsize", &[ret, p]);
    let imm: &[Attr] = &[Attr::Flag(ImmArg)];
    declare(
        m,
        &name,
        ret,
        &[(p, &[]), (TypeId::I1, imm), (TypeId::I1, imm), (TypeId::I1, imm)],
        &[],
        Shape::Pure,
    )
}

/// `i1 @llvm.is.fpclass.T(T, i32 immarg test)` for a floating-point `T`.
pub fn is_fpclass(m: &mut Module, ty: TypeId) -> FuncId {
    let name = overloaded_name(m, "llvm.is.fpclass", &[ty]);
    declare(
        m,
        &name,
        TypeId::I1,
        &[(ty, &[]), (TypeId::I32, &[Attr::Flag(ImmArg)])],
        &[],
        Shape::Pure,
    )
}

/// `{ T, E } @llvm.frexp.T.E(T)`: the fraction and the exponent of a
/// floating-point `T`, the exponent as integer type `exp`.
pub fn frexp(m: &mut Module, ty: TypeId, exp: TypeId) -> FuncId {
    let name = overloaded_name(m, "llvm.frexp", &[ty, exp]);
    let ret = m.types.struct_literal(&[ty, exp], false);
    declare(m, &name, ret, &[(ty, &[])], &[], Shape::Pure)
}

fn dbg(m: &mut Module, name: &str) -> FuncId {
    let md = TypeId::METADATA;
    declare(m, name, TypeId::VOID, &[(md, &[]), (md, &[]), (md, &[])], &[], Shape::Pure)
}

/// `void @llvm.dbg.declare(metadata, metadata, metadata)` (intrinsic form;
/// LLVM 19's printer would show debug records instead, see §6).
pub fn dbg_declare(m: &mut Module) -> FuncId {
    dbg(m, "llvm.dbg.declare")
}

/// `void @llvm.dbg.value(metadata, metadata, metadata)`.
pub fn dbg_value(m: &mut Module) -> FuncId {
    dbg(m, "llvm.dbg.value")
}

/// The switched-resume coroutine intrinsics (LLVM `Coroutines.rst`; the
/// declarations clang 19 emits for C++20 coroutines, spec/llvm.md §6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Coro {
    /// `token @llvm.coro.id(i32, ptr readnone, ptr nocapture readonly, ptr)`.
    Id,
    /// `i1 @llvm.coro.alloc(token)`.
    Alloc,
    /// `iN @llvm.coro.size.iN()` (the operand of [`coro`] is `iN`).
    Size,
    /// `iN @llvm.coro.align.iN()`.
    Align,
    /// `ptr @llvm.coro.begin(token, ptr writeonly)`.
    Begin,
    /// `ptr @llvm.coro.frame()`.
    Frame,
    /// `token @llvm.coro.save(ptr)`.
    Save,
    /// `i8 @llvm.coro.suspend(token, i1)`.
    Suspend,
    /// `i1 @llvm.coro.end(ptr, i1, token)` (LLVM 19).
    End,
    /// `void @llvm.coro.end(ptr, i1, token)` (LLVM 22, where the "in the
    /// ramp function" result moved to [`Coro::IsInRamp`]).
    EndVoid,
    /// `i1 @llvm.coro.is_in_ramp()` (LLVM 22).
    IsInRamp,
    /// `ptr @llvm.coro.free(token, ptr nocapture readonly)`.
    Free,
    /// `void @llvm.coro.resume(ptr)`.
    Resume,
    /// `void @llvm.coro.destroy(ptr)`.
    Destroy,
    /// `i1 @llvm.coro.done(ptr nocapture readonly)`.
    Done,
    /// `ptr @llvm.coro.promise(ptr nocapture, i32, i1)`.
    Promise,
    /// `ptr @llvm.coro.noop()`.
    Noop,
    /// `void @llvm.coro.await.suspend.void(ptr, ptr, ptr)`: calls the
    /// `await_suspend` wrapper (third operand) on the awaiter and the
    /// frame once the coroutine is suspended (LLVM 19).
    AwaitSuspendVoid,
    /// `i1 @llvm.coro.await.suspend.bool(ptr, ptr, ptr)`.
    AwaitSuspendBool,
    /// `void @llvm.coro.await.suspend.handle(ptr, ptr, ptr)`: `CoroSplit`
    /// resumes the handle the wrapper returns with a `musttail` call.
    AwaitSuspendHandle,
}

/// Declares (or finds) coroutine intrinsic `which`; `size` is the
/// `size_t` type the [`Coro::Size`]/[`Coro::Align`] overloads return.
pub fn coro(m: &mut Module, which: Coro, size: TypeId) -> FuncId {
    let p = ptr(m);
    let t = TypeId::TOKEN;
    let f = Attr::Flag;
    match which {
        Coro::Id => declare(
            m,
            "llvm.coro.id",
            t,
            &[
                (TypeId::I32, &[]),
                (p, &[f(AttrKind::ReadNone)]),
                (p, &[f(NoCapture), f(ReadOnly)]),
                (p, &[]),
            ],
            &[],
            Shape::Basic(Some(MemoryEffects::argmem_only(ModRef::Read))),
        ),
        Coro::Alloc => {
            declare(m, "llvm.coro.alloc", TypeId::I1, &[(t, &[])], &[], Shape::NoUnwind(false))
        }
        Coro::Size | Coro::Align => {
            let base = if which == Coro::Size { "llvm.coro.size" } else { "llvm.coro.align" };
            let name = overloaded_name(m, base, &[size]);
            declare(m, &name, size, &[], &[], Shape::NoUnwind(true))
        }
        Coro::Begin => declare(
            m,
            "llvm.coro.begin",
            p,
            &[(t, &[]), (p, &[f(WriteOnly)])],
            &[],
            Shape::NoUnwind(false),
        ),
        Coro::Frame => declare(m, "llvm.coro.frame", p, &[], &[], Shape::NoUnwind(true)),
        Coro::Save => declare(m, "llvm.coro.save", t, &[(p, &[])], &[], Shape::NoMerge),
        Coro::Suspend => declare(
            m,
            "llvm.coro.suspend",
            TypeId::I8,
            &[(t, &[]), (TypeId::I1, &[])],
            &[],
            Shape::NoUnwind(false),
        ),
        Coro::End => declare(
            m,
            "llvm.coro.end",
            TypeId::I1,
            &[(p, &[]), (TypeId::I1, &[]), (t, &[])],
            &[],
            Shape::NoUnwind(false),
        ),
        Coro::EndVoid => declare(
            m,
            "llvm.coro.end",
            TypeId::VOID,
            &[(p, &[]), (TypeId::I1, &[]), (t, &[])],
            &[],
            Shape::NoUnwind(false),
        ),
        Coro::IsInRamp => {
            declare(m, "llvm.coro.is_in_ramp", TypeId::I1, &[], &[], Shape::NoUnwind(true))
        }
        Coro::Free => declare(
            m,
            "llvm.coro.free",
            p,
            &[(t, &[]), (p, &[f(NoCapture), f(ReadOnly)])],
            &[],
            Shape::NoUnwindArgRead,
        ),
        Coro::Resume => {
            declare(m, "llvm.coro.resume", TypeId::VOID, &[(p, &[])], &[], Shape::Throws)
        }
        Coro::Destroy => {
            declare(m, "llvm.coro.destroy", TypeId::VOID, &[(p, &[])], &[], Shape::Throws)
        }
        Coro::Done => declare(
            m,
            "llvm.coro.done",
            TypeId::I1,
            &[(p, &[f(NoCapture), f(ReadOnly)])],
            &[],
            Shape::NoUnwindArgRead,
        ),
        Coro::Promise => declare(
            m,
            "llvm.coro.promise",
            p,
            &[(p, &[f(NoCapture)]), (TypeId::I32, &[]), (TypeId::I1, &[])],
            &[],
            Shape::NoUnwind(true),
        ),
        Coro::Noop => declare(m, "llvm.coro.noop", p, &[], &[], Shape::NoUnwind(true)),
        Coro::AwaitSuspendVoid | Coro::AwaitSuspendBool | Coro::AwaitSuspendHandle => {
            coro_await_suspend(m, which)
        }
    }
}

/// The three `llvm.coro.await.suspend.*` intrinsics of [`coro`].
fn coro_await_suspend(m: &mut Module, which: Coro) -> FuncId {
    let p = ptr(m);
    let (name, ret) = match which {
        Coro::AwaitSuspendVoid => ("llvm.coro.await.suspend.void", TypeId::VOID),
        Coro::AwaitSuspendBool => ("llvm.coro.await.suspend.bool", TypeId::I1),
        _ => ("llvm.coro.await.suspend.handle", TypeId::VOID),
    };
    declare(m, name, ret, &[(p, &[]), (p, &[]), (p, &[])], &[], Shape::Throws)
}
