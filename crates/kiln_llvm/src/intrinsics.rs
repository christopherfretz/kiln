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
    let fa = m.attr_set(&fn_attrs(shape));
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
    declare(
        m,
        &name,
        TypeId::VOID,
        &[(TypeId::I64, &[Attr::Flag(ImmArg)]), (p, &[Attr::Flag(NoCapture)])],
        &[],
        Shape::NoSyncArgMem,
    )
}

/// `void @llvm.lifetime.start.p0(i64 immarg size, ptr nocapture)`.
pub fn lifetime_start(m: &mut Module) -> FuncId {
    lifetime(m, "llvm.lifetime.start")
}

/// `void @llvm.lifetime.end.p0(i64 immarg size, ptr nocapture)`.
pub fn lifetime_end(m: &mut Module) -> FuncId {
    lifetime(m, "llvm.lifetime.end")
}

/// `void @llvm.trap()`.
pub fn trap(m: &mut Module) -> FuncId {
    declare(m, "llvm.trap", TypeId::VOID, &[], &[], Shape::Trap)
}

/// `void @llvm.debugtrap()`.
pub fn debugtrap(m: &mut Module) -> FuncId {
    declare(m, "llvm.debugtrap", TypeId::VOID, &[], &[], Shape::NoUnwind(false))
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
