//! Builtin function calls (spec/codegen.md §3, "Calls"): clang `EmitBuiltinExpr`
//! for the builtins kiln declares (`kiln_ast::decl::builtin`).
//!
//! A builtin is never emitted as a function. A call whose value folds to an
//! integer or floating constant is that constant (clang tries
//! `EvaluateAsRValue` before anything else; the fold prepass collects these
//! calls). The rest lower to LLVM intrinsics, inline IR, or a call to the
//! library function the builtin names (`__builtin_strlen` → `strlen`).

use kiln_ast::decl::builtin as id;
use kiln_ast::decl::{DeclKind, builtin::attr};
use kiln_ast::expr::{CastKind, ExprKind, UnaryOp};
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::{Ast, DeclId, ExprId, QualType};
use kiln_llvm::{
    Attr, AttrKind, AttrList, BinOp, CallOpts, CastOp, Flags, FloatPredicate, FuncId, IntPredicate,
    MemoryEffects, ModRef, TypeId as LTy, Value,
};

use crate::fir::{Fir, Op, V};
use crate::func::{Addr, CallArg};
use crate::module::{Cg, R, err};

/// The builtin `e` calls directly, if it is a call of one: its declaration
/// and id.
pub(crate) fn builtin_callee(ast: &Ast, e: ExprId) -> Option<(DeclId, u32)> {
    let mut c = ast.exprs.callee(e);
    loop {
        match ast.exprs.get(c).kind() {
            ExprKind::ParenExpr => c = ast.exprs.operand(c),
            ExprKind::ImplicitCastExpr
                if matches!(
                    ast.exprs.cast_kind(c),
                    CastKind::FunctionToPointerDecay | CastKind::BuiltinFnToFnPtr
                ) =>
            {
                c = ast.exprs.operand(c);
            }
            ExprKind::DeclRefExpr => {
                let d = ast.exprs.decl(c);
                if ast.decls.get(d).kind() != DeclKind::Function {
                    return None;
                }
                return ast.decls.builtin_id(d).map(|i| (d, i));
            }
            _ => return None,
        }
    }
}

/// Whether the call `e` of a builtin is folded when it can be (clang
/// `EmitBuiltinExpr` keeps an `EvaluateAsRValue` result that is an integer
/// or a floating value).
pub(crate) fn is_fold_candidate(ast: &Ast, e: ExprId, compat: u32) -> bool {
    let q = ast.exprs.get(e).ty();
    // `__builtin_is_constant_evaluated()` is false outside a manifestly
    // constant-evaluated context, which a fold is not.
    // A checked arithmetic (or carry, WP-557) builtin stores through its
    // pointer argument, so it is emitted.
    // WP-365: the `<cmath>` builtins the Oven evaluates and clang 19 does
    // not are emitted as clang emits them.
    builtin_callee(ast, e).is_some_and(|(_, b)| {
        b != id::IS_CONSTANT_EVALUATED
            && b != id::KILN_PLAINLY_CONSTANT_EVALUATED
            && id::checked_op(b).is_none()
            && id::carry_op(b).is_none()
            // WP-630: nor an x86 builtin with a pointer operand
            // (`addcarryx` stores its sum).
            && !id::x86::row(b).is_some_and(|r| r.info.sig.contains('*'))
            && !clang19_unevaluated(b)
            // clang 20 made `abs`/`labs`/`llabs` constant (WP-432).
            && !(compat < 22 && matches!(b, id::ABS | id::LABS | id::LLABS))
    }) && (ast.types.is_integral(q) || ast.types.is_floating(q))
}

/// The builtins kiln constant-evaluates (WP-365, spec/oven.md
/// "Floating-point math builtins") that clang 19 does not.
fn clang19_unevaluated(b: u32) -> bool {
    id::p0533_base(b).is_some()
        || matches!(
            b,
            id::ISGREATER
                | id::ISGREATEREQUAL
                | id::ISLESS
                | id::ISLESSEQUAL
                | id::ISLESSGREATER
                | id::ISUNORDERED
                | id::COPYSIGNF16
                // WP-403: clang 19 and 22 do not fold `fabsf16`.
                | id::FABSF16
        )
}

/// The arguments of the builtin call `e` codegen folds rather than emits
/// (the `__builtin_isfpclass` mask, the `__builtin_object_size` type,
/// the `__builtin_assume_aligned` alignment, an x86 builtin's integer
/// constant operands, a NEON builtin's lane and type-code immediates), for
/// the fold prepass.
pub(crate) fn folded_args(ast: &Ast, e: ExprId) -> Vec<ExprId> {
    let Some((_, b)) = builtin_callee(ast, e) else { return Vec::new() };
    let args = || ast.lists.iter(ast.exprs.call_args(e));
    if id::x86::is_x86(b) {
        let ice = id::ice_params(b);
        return args()
            .enumerate()
            .filter(|&(i, _)| i < 32 && ice & (1 << i) != 0)
            .map(|x| x.1)
            .collect();
    }
    // `__builtin_expect_with_probability`'s probability is evaluated, not
    // emitted (clang `EvaluateAsFloat`).
    let idx: &'static [usize] = match b {
        id::ISFPCLASS | id::OBJECT_SIZE | id::DYNAMIC_OBJECT_SIZE | id::ASSUME_ALIGNED => &[1],
        id::EXPECT_WITH_PROBABILITY => &[2],
        _ => crate::neon::imm_args(b),
    };
    idx.iter().filter_map(|&i| args().nth(i)).collect()
}

/// The name of the library function a library builtin calls (clang
/// `getBuiltinLibFunction`), `None` for a builtin that is not one.
fn lib_name(b: u32) -> Option<&'static str> {
    let b = if b == id::CHAR_MEMCHR { id::MEMCHR } else { b };
    match b {
        id::MEMCMP
        | id::MEMCHR
        | id::STRLEN
        | id::WCSLEN
        | id::WMEMCHR
        | id::WMEMCMP
        | id::STRCMP
        | id::STRNCMP
        | id::STRCHR
        | id::STRRCHR
        | id::STRPBRK
        | id::STRSTR
        | id::WCSCMP
        | id::WCSNCMP
        | id::WCSCHR
        | id::NAN
        | id::NANF
        | id::NANL
        | id::NANS
        | id::NANSF
        | id::NANSL
        | id::ABORT
        | id::BCMP
        | id::WMEMCPY
        | id::WMEMMOVE
        | id::SNPRINTF
        | id::SPRINTF
        | id::VSNPRINTF => id::info(b).and_then(|i| i.name.strip_prefix("__builtin_")),
        _ if id::is_libm(b) => id::info(b).and_then(|i| i.name.strip_prefix("__builtin_")),
        _ => None,
    }
}

/// The elementwise LLVM floating-point intrinsic a builtin lowers to
/// (clang `emitBuiltinWithOneOverloadedType`) and its operand count.
fn fp_intrinsic(b: u32) -> Option<(&'static str, usize)> {
    Some(match b {
        id::FABS | id::FABSF | id::FABSL => ("llvm.fabs", 1),
        id::COPYSIGN | id::COPYSIGNF | id::COPYSIGNL => ("llvm.copysign", 2),
        id::FMAX | id::FMAXF | id::FMAXL => ("llvm.maxnum", 2),
        id::FMIN | id::FMINF | id::FMINL => ("llvm.minnum", 2),
        id::CEIL | id::CEILF | id::CEILL => ("llvm.ceil", 1),
        id::FLOOR | id::FLOORF | id::FLOORL => ("llvm.floor", 1),
        id::TRUNC | id::TRUNCF | id::TRUNCL => ("llvm.trunc", 1),
        id::ROUND | id::ROUNDF | id::ROUNDL => ("llvm.round", 1),
        id::RINT | id::RINTF | id::RINTL => ("llvm.rint", 1),
        id::NEARBYINT | id::NEARBYINTF | id::NEARBYINTL => ("llvm.nearbyint", 1),
        id::FMA | id::FMAF | id::FMAL => ("llvm.fma", 3),
        // WP-282: `_Float16` / `__float128` forms without a `long double`
        // twin ([`id::f128_twin`] maps the others).
        id::COPYSIGNF16 => ("llvm.copysign", 2),
        id::ROUNDEVENF128 => ("llvm.roundeven", 1),
        // WP-403: the `__fp16` ones without a `float` twin.
        id::ROUNDEVENF16 => ("llvm.roundeven", 1),
        id::CANONICALIZEF16 => ("llvm.canonicalize", 1),
        id::FMAXIMUM_NUMF16 => ("llvm.maximumnum", 2),
        id::FMINIMUM_NUMF16 => ("llvm.minimumnum", 2),
        // WP-410.
        id::ROUNDEVEN | id::ROUNDEVENF | id::ROUNDEVENL => ("llvm.roundeven", 1),
        id::CANONICALIZE | id::CANONICALIZEF | id::CANONICALIZEL => ("llvm.canonicalize", 1),
        id::FMAXIMUM_NUM | id::FMAXIMUM_NUMF | id::FMAXIMUM_NUML => ("llvm.maximumnum", 2),
        id::FMINIMUM_NUM | id::FMINIMUM_NUMF | id::FMINIMUM_NUML => ("llvm.minimumnum", 2),
        _ => return None,
    })
}

/// WP-419: the `__builtin_` twin a call of the recognized C library
/// function `d` lowers as (clang `EmitBuiltinExpr` for a library builtin
/// whose `__builtin_` form is an LLVM intrinsic: `ceil(x)` ->
/// `llvm.ceil`), if any. Under clang's default `-fmath-errno` only the
/// errno-free functions are intrinsics, which are exactly the
/// [`fp_intrinsic`] ones; WP-186 adds `memcpy` / `memmove` / `memset`
/// and the integer `abs` family, which clang lowers through their
/// builtin id too (`memcpy` -> `llvm.memcpy`, spec/codegen.md §3). The
/// others (`strlen`, `sqrt`, `cbrt`, ...) stay library calls.
pub(crate) fn lib_intrinsic(ast: &Ast, d: DeclId) -> Option<u32> {
    let t = ast.decls.lib_builtin(d)?;
    let mem = matches!(t, id::MEMCPY | id::MEMMOVE | id::MEMSET | id::ABS | id::LABS | id::LLABS);
    (mem || fp_intrinsic(t).is_some()).then_some(t)
}

/// The `llvm.is.fpclass` test of a classification builtin
/// (`llvm::FPClassTest`).
fn fp_class_test(b: u32) -> Option<i128> {
    Some(match b {
        id::ISNAN => 0x003,
        id::ISINF => 0x204,
        id::ISNORMAL => 0x108,
        id::ISFINITE => 0x1f8,
        id::ISSUBNORMAL => 0x090,
        id::ISZERO => 0x060,
        id::ISSIGNALING => 0x001,
        _ => return None,
    })
}

/// The ordered (or unordered) comparison of an `isgreater`-family builtin.
fn fp_compare(b: u32) -> Option<FloatPredicate> {
    Some(match b {
        id::ISGREATER => FloatPredicate::Ogt,
        id::ISGREATEREQUAL => FloatPredicate::Oge,
        id::ISLESS => FloatPredicate::Olt,
        id::ISLESSEQUAL => FloatPredicate::Ole,
        id::ISLESSGREATER => FloatPredicate::One,
        id::ISUNORDERED => FloatPredicate::Uno,
        _ => return None,
    })
}

/// The extra function attributes of a library builtin's declaration and
/// call sites: `Pure` and `Const` (clang `ConstructAttributeList`). A
/// return or argument passed by address (`indirect`; `__builtin_cprojl`'s
/// x86-64 `long double _Complex`) adds `argmem: readwrite` (clang
/// `AddPotentialArgAccess`).
fn lib_attrs(b: u32, indirect: bool, out: &mut Vec<Attr>) {
    let bits = id::info(b).map_or(0, |i| i.attrs);
    if bits & attr::NORETURN != 0 {
        out.push(Attr::Flag(AttrKind::NoReturn));
    }
    let mem = if bits & attr::CONST != 0 {
        ModRef::None
    } else if bits & attr::PURE != 0 {
        ModRef::Read
    } else {
        return;
    };
    let mut effects = MemoryEffects::all(mem);
    if indirect {
        effects.argmem = ModRef::ReadWrite;
    }
    out.push(Attr::Flag(AttrKind::WillReturn));
    out.push(Attr::Memory(effects));
}

/// The function attributes a written or implicit `const` / `pure`
/// attribute on some declaration of `d` gives its declaration and call
/// sites (clang `ConstructAttributeList`: `nounwind willreturn` plus
/// `memory(none)` / `memory(read)`); none for a builtin, whose
/// [`lib_attrs`] already say so. A return or argument passed by
/// address (`sig`) adds `argmem: readwrite` (clang
/// `AddPotentialArgAccess`). Returns whether it added any. A GNU
/// `cold` / `hot` attribute adds `cold` / `hot` the same way (WP-172);
/// it does not count toward the result.
pub(crate) fn const_pure_attrs(
    ast: &Ast,
    d: DeclId,
    sig: &crate::module::Sig,
    out: &mut Vec<Attr>,
) -> bool {
    if ast.decls.builtin_id(d).is_some() {
        return false;
    }
    let has = |k: kiln_ast::attr::AttrKind| {
        ast.redecls_of(d).any(|r| {
            ast.lists
                .iter::<kiln_ast::AttrId>(ast.attrs.of_decl(r))
                .any(|a| ast.attrs.get(a).kind == k)
        })
    };
    cold_hot_attrs(ast, d, out);
    let mem = if has(kiln_ast::attr::AttrKind::CONST) {
        ModRef::None
    } else if has(kiln_ast::attr::AttrKind::PURE) {
        ModRef::Read
    } else {
        return false;
    };
    let mut effects = MemoryEffects::all(mem);
    if sig.passes_indirect() {
        effects.argmem = ModRef::ReadWrite;
    }
    out.push(Attr::Flag(AttrKind::WillReturn));
    out.push(Attr::Memory(effects));
    true
}

/// The `cold` / `hot` function attributes a GNU `cold` / `hot` attribute
/// on some declaration of `d` gives its declaration and direct call sites
/// (clang `ConstructAttributeList`; WP-172).
pub(crate) fn cold_hot_attrs(ast: &Ast, d: DeclId, out: &mut Vec<Attr>) {
    let has = |k: kiln_ast::attr::AttrKind| {
        ast.redecls_of(d).any(|r| {
            ast.lists
                .iter::<kiln_ast::AttrId>(ast.attrs.of_decl(r))
                .any(|a| ast.attrs.get(a).kind == k)
        })
    };
    if has(kiln_ast::attr::AttrKind::COLD) {
        out.push(Attr::Flag(AttrKind::Cold));
    }
    if has(kiln_ast::attr::AttrKind::HOT) {
        out.push(Attr::Flag(AttrKind::Hot));
    }
}

/// Whether `q` is a complete object type (an incomplete class has no
/// natural alignment to speak of).
pub(crate) fn is_complete(ast: &Ast, q: QualType) -> bool {
    ast.types.as_record(q).is_none_or(|r| {
        ast.definition(r).is_some_and(|d| {
            ast.decls
                .record(d)
                .is_some_and(|x| x.defn.contains(kiln_ast::decl::DefinitionBits::COMPLETE))
        })
    })
}

/// How [`Cg::emit_builtin`] handled a builtin call.
pub(crate) enum BuiltinCall {
    /// Emitted inline: the value, or none for a `void` call.
    Emitted(Option<V>),
    /// A library builtin: the caller emits an ordinary call.
    Library,
}

impl Cg<'_> {
    /// Whether `d` is one of the standard functions clang treats as a
    /// builtin and folds at every optimization level (spec/codegen.md §3,
    /// "Calls"; clang `isStdBuiltin` + `CheckBuiltinFunctionCall`):
    /// `std::move`, `move_if_noexcept`, `forward`, `forward_like`,
    /// `as_const`, `addressof` and `__addressof`, declared in namespace
    /// `std` (through inline namespaces) with one non-variadic parameter of
    /// reference type, returning a reference or pointer to the same type up
    /// to qualifiers. A call of one is the address of its argument's
    /// lvalue; the function itself is not referenced.
    pub(crate) fn is_std_builtin(&self, d: DeclId) -> bool {
        const NAMES: [&str; 7] = [
            "move",
            "move_if_noexcept",
            "forward",
            "forward_like",
            "as_const",
            "addressof",
            "__addressof",
        ];
        let ast = self.ast;
        if ast.decls.get(d).kind() != DeclKind::Function || !NAMES.iter().any(|n| self.in_std(d, n))
        {
            return false;
        }
        let Some(fty) = ast.decls.get(d).ty() else { return false };
        let Some(p) = ast.types.proto(fty) else { return false };
        let [param] = ast.types.params(p) else { return false };
        if p.variadic() || !ast.types.is_reference(*param) {
            return false;
        }
        let ret = p.ret();
        (ast.types.is_reference(ret) || ast.types.is_pointer(ret))
            && match (ast.types.pointee(*param), ast.types.pointee(ret)) {
                (Some(a), Some(b)) => ast.types.same_unqualified(a, b),
                _ => false,
            }
    }

    /// clang `EmitVAListRef`: the address of the `va_list` operand `x`, a
    /// `__builtin_va_list` lvalue, or the decayed pointer when the
    /// target's `va_list` is an array (x86-64).
    fn va_list_ref(&mut self, x: ExprId) -> R<V> {
        if self.ast.types.is_pointer(self.ast.exprs.get(x).ty()) {
            self.emit_scalar(x)
        } else {
            Ok(self.emit_lvalue(x)?.addr.v)
        }
    }

    /// clang `EmitBuiltinExpr` for the call `e` of builtin `b`:
    /// [`BuiltinCall::Library`] for a library builtin, which the caller
    /// emits as an ordinary call of [`Cg::lib_fn`].
    pub(crate) fn emit_builtin(&mut self, b: u32, e: ExprId) -> R<BuiltinCall> {
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
        if let Some(c) = self.folds.get(e)
            && (ast.types.is_integral(q) || ast.types.is_floating(q))
        {
            return Ok(BuiltinCall::Emitted(Some(V::C(self.const_scalar_value(c, q)?))));
        }
        let args: Vec<ExprId> = ast.lists.iter(ast.exprs.call_args(e)).collect();
        let arg = |i: usize| args.get(i).copied();
        let Some(a0) = arg(0).or(Some(e)) else { unreachable!() };
        // WP-282: a `__float128` builtin with a `long double` twin lowers
        // as the twin (the operand types pick the intrinsic overload).
        let b = id::f128_twin(b).unwrap_or(b);
        if id::is_coro(b) {
            return self.emit_coro_builtin(b, e);
        }
        if id::neon::is_neon(b) {
            return self.emit_neon_builtin(b, e);
        }
        if id::x86::is_x86(b) {
            return self.emit_x86_builtin(b, e);
        }
        if id::arm::is_arm(b) {
            return self.emit_arm_builtin(b, e);
        }
        let v = match b {
            id::IS_CONSTANT_EVALUATED | id::KILN_PLAINLY_CONSTANT_EVALUATED => {
                V::C(self.m.const_bool(false))
            }
            // clang `Builtin::BI__builtin_complex`: the two parts.
            id::COMPLEX => {
                let re = self.emit_scalar(a0)?;
                let Some(a1) = arg(1) else {
                    return err(self.loc_of(e), "__builtin_complex needs two arguments");
                };
                let im = self.emit_scalar(a1)?;
                self.f.cx_ret = Some((re, im));
                return Ok(BuiltinCall::Emitted(None));
            }
            // An argument that did not fold (the fold prepass made the call
            // 1 or 0 when it could evaluate it) is not a constant, as at
            // clang -O0; it is not evaluated.
            id::CONSTANT_P => V::C(self.m.const_int(LTy::I32, 0)),
            // WP-419: the operand's GCC type class, the operand not
            // evaluated (clang folds the call whatever the operand).
            id::CLASSIFY_TYPE => {
                let c = arg(0).map_or(-1, |x| ast.gcc_type_class(ast.exprs.get(x).ty()));
                V::C(self.m.const_int(LTy::I32, i128::from(c)))
            }
            // [specialized.addressof]: `&x` without an overloaded `&`.
            id::ADDRESSOF => self.emit_lvalue(a0)?.addr.v,
            id::CLZ
            | id::CLZL
            | id::CLZLL
            | id::CLZG
            | id::CTZ
            | id::CTZL
            | id::CTZLL
            | id::CTZG => {
                let ctz = matches!(b, id::CTZ | id::CTZL | id::CTZLL | id::CTZG);
                let fallback = if matches!(b, id::CLZG | id::CTZG) { arg(1) } else { None };
                let x = self.emit_scalar(a0)?;
                let ty = self.f.fir.ty(&self.m, x);
                let f = if ctz {
                    kiln_llvm::intrinsics::cttz(&mut self.m, ty)
                } else {
                    kiln_llvm::intrinsics::ctlz(&mut self.m, ty)
                };
                // `TargetInfo::isCLZForZeroUndef`: false only on AArch64.
                let zero_undef = self.m.const_bool(fallback.is_some() || self.is_x86());
                let r = self.intrinsic_call(f, vec![x, V::C(zero_undef)]);
                let rt = self.lty(q);
                let r = self.int_cast(r, rt, false);
                match fallback {
                    None => r,
                    Some(fb) => {
                        let zero = V::C(self.m.const_int(ty, 0));
                        let is_zero = self.icmp(IntPredicate::Eq, x, zero);
                        let fv = self.emit_scalar(fb)?;
                        self.select(is_zero, fv, r)
                    }
                }
            }
            // WP-156: clang `emitBuiltinWithOneOverloadedType` of
            // `llvm.vector.reduce.*` (min/max by element signedness).
            id::REDUCE_ADD
            | id::REDUCE_MUL
            | id::REDUCE_AND
            | id::REDUCE_OR
            | id::REDUCE_XOR
            | id::REDUCE_MIN
            | id::REDUCE_MAX => {
                let x = self.emit_scalar(a0)?;
                let ty = self.f.fir.ty(&self.m, x);
                let aq = ast.exprs.get(a0).ty();
                let eq = self.scalar_of(aq);
                let max = b == id::REDUCE_MAX;
                let op = match b {
                    id::REDUCE_ADD => "add",
                    id::REDUCE_MUL => "mul",
                    id::REDUCE_AND => "and",
                    id::REDUCE_OR => "or",
                    id::REDUCE_XOR => "xor",
                    _ if ast.types.is_floating(eq) => {
                        if max {
                            "fmax"
                        } else {
                            "fmin"
                        }
                    }
                    _ if self.is_signed(eq) && !self.is_bool(eq) => {
                        if max {
                            "smax"
                        } else {
                            "smin"
                        }
                    }
                    _ if max => "umax",
                    _ => "umin",
                };
                let f = kiln_llvm::intrinsics::vector_reduce(&mut self.m, op, ty);
                self.intrinsic_call(f, vec![x])
            }
            id::POPCOUNT | id::POPCOUNTL | id::POPCOUNTLL | id::POPCOUNTG => {
                let x = self.emit_scalar(a0)?;
                let ty = self.f.fir.ty(&self.m, x);
                let f = kiln_llvm::intrinsics::ctpop(&mut self.m, ty);
                let r = self.intrinsic_call(f, vec![x]);
                let rt = self.lty(q);
                self.int_cast(r, rt, false)
            }
            id::BSWAP16 | id::BSWAP32 | id::BSWAP64 => {
                let x = self.emit_scalar(a0)?;
                let ty = self.f.fir.ty(&self.m, x);
                let f = kiln_llvm::intrinsics::bswap(&mut self.m, ty);
                self.intrinsic_call(f, vec![x])
            }
            id::ISNAN
            | id::ISINF
            | id::ISNORMAL
            | id::ISFINITE
            | id::ISSUBNORMAL
            | id::ISZERO
            | id::ISSIGNALING => {
                let x = self.emit_scalar(a0)?;
                let ty = self.f.fir.ty(&self.m, x);
                let f = kiln_llvm::intrinsics::is_fpclass(&mut self.m, ty);
                let test = fp_class_test(b).unwrap_or_default();
                let t = V::C(self.m.const_int(LTy::I32, test));
                let r = self.intrinsic_call(f, vec![x, t]);
                let rt = self.lty(q);
                self.cast(CastOp::ZExt, r, rt)
            }
            id::ISFPCLASS => {
                // clang: the mask is an integer constant, folded, not emitted.
                let Some(m) = arg(1) else {
                    return err(self.loc_of(e), "__builtin_isfpclass without a mask");
                };
                let Some(test) = self.fold_int(m) else {
                    return err(self.loc_of(e), "__builtin_isfpclass mask is not a constant");
                };
                let x = self.emit_scalar(a0)?;
                let ty = self.f.fir.ty(&self.m, x);
                let f = kiln_llvm::intrinsics::is_fpclass(&mut self.m, ty);
                let t = V::C(self.m.const_int(LTy::I32, test));
                let r = self.intrinsic_call(f, vec![x, t]);
                let rt = self.lty(q);
                self.cast(CastOp::ZExt, r, rt)
            }
            id::SIGNBIT => {
                // clang `EmitSignBit`: the sign of the bits as an integer.
                let x = self.emit_scalar(a0)?;
                let ty = self.f.fir.ty(&self.m, x);
                let w = self.m.types.float_width(ty).unwrap_or(64);
                let it = self.m.types.int(w);
                let bits = self.cast(CastOp::BitCast, x, it);
                let zero = V::C(self.m.const_int(it, 0));
                let r = self.icmp(IntPredicate::Slt, bits, zero);
                let rt = self.lty(q);
                self.cast(CastOp::ZExt, r, rt)
            }
            id::ISGREATER
            | id::ISGREATEREQUAL
            | id::ISLESS
            | id::ISLESSEQUAL
            | id::ISLESSGREATER
            | id::ISUNORDERED => {
                let (Some(l), Some(r)) = (arg(0), arg(1)) else {
                    return err(self.loc_of(e), "comparison builtin without two arguments");
                };
                let l = self.emit_scalar(l)?;
                let r = self.emit_scalar(r)?;
                let p = fp_compare(b).unwrap_or(FloatPredicate::Uno);
                let c = self.fcmp(p, l, r);
                let rt = self.lty(q);
                self.cast(CastOp::ZExt, c, rt)
            }
            id::FREXP | id::FREXPF | id::FREXPL => {
                // clang `emitFrexpBuiltin`: `llvm.frexp`, the exponent
                // stored through the pointer at its natural alignment.
                let (Some(x), Some(p)) = (arg(0), arg(1)) else {
                    return err(self.loc_of(e), "frexp without two arguments");
                };
                let x = self.emit_scalar(x)?;
                let pq = ast.exprs.get(p).ty();
                let p = self.emit_scalar(p)?;
                let iq = ast.types.pointee(pq).unwrap_or(QualType::ERROR);
                let ty = self.f.fir.ty(&self.m, x);
                let it = self.lty(iq);
                let f = kiln_llvm::intrinsics::frexp(&mut self.m, ty, it);
                let rty = self.m.types.struct_literal(&[ty, it], false);
                let call = self.intrinsic_call_ty(f, vec![x], rty);
                let exp = self.f.fir.push(Op::ExtractValue { agg: call, idx: 1 }, it);
                let align = self.size_align(iq).1;
                let mt = self.mty(iq);
                self.store_scalar(exp, Addr { v: p, ty: mt, align }, iq);
                self.f.fir.push(Op::ExtractValue { agg: call, idx: 0 }, ty)
            }
            id::NEXTAFTERF16 | id::NEXTAFTERF16B => {
                let Some(a1) = arg(1) else {
                    return err(self.loc_of(e), "nextafter without two arguments");
                };
                let x = self.emit_scalar(a0)?;
                let y = self.emit_scalar(a1)?;
                self.nextafter16(x, y)
            }
            _ if fp_intrinsic(b).is_some() => {
                let (name, n) = fp_intrinsic(b).unwrap_or(("llvm.fabs", 1));
                let mut vs = Vec::with_capacity(n);
                for i in 0..n {
                    let Some(a) = arg(i) else {
                        return err(self.loc_of(e), "math builtin with too few arguments");
                    };
                    vs.push(self.emit_scalar(a)?);
                }
                let ty = self.f.fir.ty(&self.m, vs[0]);
                let f = kiln_llvm::intrinsics::fp_op(&mut self.m, name, ty, n);
                self.intrinsic_call(f, vs)
            }
            id::ABS | id::LABS | id::LLABS => {
                // clang 19: `llvm.abs(x, true)` (signed overflow is UB).
                let x = self.emit_scalar(a0)?;
                let ty = self.f.fir.ty(&self.m, x);
                let f = kiln_llvm::intrinsics::abs(&mut self.m, ty);
                let t = V::C(self.m.const_bool(true));
                self.intrinsic_call(f, vec![x, t])
            }
            // clang `EmitBuiltinAlignTo` / `BI__builtin_is_aligned`: the
            // alignment zero-extended or truncated to the source integer
            // type (the index type for a pointer), `mask = align - 1`.
            id::ALIGN_UP | id::ALIGN_DOWN | id::IS_ALIGNED => {
                let Some(a1) = arg(1) else {
                    return err(self.loc_of(e), "alignment builtin without two arguments");
                };
                let src = self.emit_scalar(a0)?;
                let al = self.emit_scalar(a1)?;
                let sty = self.f.fir.ty(&self.m, src);
                let is_ptr = self.m.types.int_width(sty).is_none();
                let ity = if is_ptr { LTy::I64 } else { sty };
                let al = self.int_cast(al, ity, false);
                let one = V::C(self.m.const_int(ity, 1));
                let mask = self.bin(BinOp::Sub, Flags::NONE, al, one);
                if b == id::IS_ALIGNED {
                    let addr = if is_ptr { self.cast(CastOp::PtrToInt, src, ity) } else { src };
                    let set = self.bin(BinOp::And, Flags::NONE, addr, mask);
                    let zero = V::C(self.m.const_int(ity, 0));
                    self.icmp(IntPredicate::Eq, set, zero)
                } else {
                    let over = match (b == id::ALIGN_UP, is_ptr) {
                        (false, _) => src,
                        (true, true) => self.gep(LTy::I8, src, vec![mask], true),
                        (true, false) => self.bin(BinOp::Add, Flags::NONE, src, mask),
                    };
                    let all = V::C(self.m.const_int(ity, -1));
                    let inv = self.bin(BinOp::Xor, Flags::NONE, mask, all);
                    if is_ptr {
                        let f = kiln_llvm::intrinsics::ptrmask(&mut self.m);
                        self.intrinsic_call(f, vec![over, inv])
                    } else {
                        self.bin(BinOp::And, Flags::NONE, over, inv)
                    }
                }
            }
            // clang `BI__builtin_frame_address`: `llvm.frameaddress(i32 N)`.
            id::FRAME_ADDRESS => {
                let level = self.emit_scalar(a0)?;
                let level = self.int_cast(level, LTy::I32, false);
                let f = kiln_llvm::intrinsics::frameaddress(&mut self.m);
                self.intrinsic_call(f, vec![level])
            }
            // clang `BI__builtin_thread_pointer`: `llvm.thread.pointer`
            // (`.p0` from LLVM 20, by the consuming LLVM's version).
            id::THREAD_POINTER => {
                let f =
                    kiln_llvm::intrinsics::thread_pointer(&mut self.m, self.opts.llvm_major >= 20);
                self.intrinsic_call(f, vec![])
            }
            // clang `BI__builtin_return_address`: `llvm.returnaddress(i32 N)`.
            id::RETURN_ADDRESS => {
                let level = self.emit_scalar(a0)?;
                let level = self.int_cast(level, LTy::I32, false);
                let f = kiln_llvm::intrinsics::returnaddress(&mut self.m);
                self.intrinsic_call(f, vec![level])
            }
            // clang `BI__builtin_elementwise_add_sat` / `_sub_sat`:
            // `llvm.{s,u}{add,sub}.sat` by element signedness.
            id::ELEMENTWISE_ADD_SAT | id::ELEMENTWISE_SUB_SAT => {
                let Some(a1) = arg(1) else {
                    return err(self.loc_of(e), "elementwise builtin without two arguments");
                };
                let x = self.emit_scalar(a0)?;
                let y = self.emit_scalar(a1)?;
                let ty = self.f.fir.ty(&self.m, x);
                let eq = self.scalar_of(ast.exprs.get(a0).ty());
                let name = match (self.is_signed(eq), b == id::ELEMENTWISE_ADD_SAT) {
                    (true, true) => "llvm.sadd.sat",
                    (true, false) => "llvm.ssub.sat",
                    (false, true) => "llvm.uadd.sat",
                    (false, false) => "llvm.usub.sat",
                };
                let f = kiln_llvm::intrinsics::fp_op(&mut self.m, name, ty, 2);
                self.intrinsic_call(f, vec![x, y])
            }
            // clang `BI__builtin_elementwise_abs`: `llvm.abs(x, false)` for
            // integers, `llvm.fabs` for floating point (lane-wise on vectors).
            id::ELEMENTWISE_ABS => {
                let x = self.emit_scalar(a0)?;
                let ty = self.f.fir.ty(&self.m, x);
                let eq = self.scalar_of(ast.exprs.get(a0).ty());
                if ast.types.is_floating(eq) {
                    let f = kiln_llvm::intrinsics::fabs(&mut self.m, ty);
                    self.intrinsic_call(f, vec![x])
                } else {
                    let f = kiln_llvm::intrinsics::abs(&mut self.m, ty);
                    let t = V::C(self.m.const_bool(false));
                    self.intrinsic_call(f, vec![x, t])
                }
            }
            // clang `BI__builtin_elementwise_max` / `_min`: `llvm.maxnum` /
            // `llvm.minnum` for floating point, else by element signedness.
            id::ELEMENTWISE_MAX | id::ELEMENTWISE_MIN => {
                let Some(a1) = arg(1) else {
                    return err(self.loc_of(e), "elementwise builtin without two arguments");
                };
                let x = self.emit_scalar(a0)?;
                let y = self.emit_scalar(a1)?;
                let ty = self.f.fir.ty(&self.m, x);
                let eq = self.scalar_of(ast.exprs.get(a0).ty());
                let max = b == id::ELEMENTWISE_MAX;
                let name = match (ast.types.is_floating(eq), self.is_signed(eq), max) {
                    (true, _, true) => "llvm.maxnum",
                    (true, _, false) => "llvm.minnum",
                    (false, true, true) => "llvm.smax",
                    (false, true, false) => "llvm.smin",
                    (false, false, true) => "llvm.umax",
                    (false, false, false) => "llvm.umin",
                };
                let f = kiln_llvm::intrinsics::fp_op(&mut self.m, name, ty, 2);
                self.intrinsic_call(f, vec![x, y])
            }
            id::INF | id::INFF | id::INFL => {
                let ty = self.lty(q);
                let w = self.m.types.float_width(ty).unwrap_or(64);
                let bits: u128 = match w {
                    32 => 0x7f80_0000,
                    16 => 0x7c00,
                    80 => 0x7fff_8000_0000_0000_0000,
                    128 => 0x7fff_u128 << 112,
                    _ => 0x7ff0_0000_0000_0000,
                };
                V::C(self.m.const_float_bits(ty, bits))
            }
            id::ISINF_SIGN => {
                // clang: isinf(x) ? (signbit(x) ? -1 : 1) : 0.
                let x = self.emit_scalar(a0)?;
                let ty = self.f.fir.ty(&self.m, x);
                let abs = self.fabs_no_mem(x, ty);
                let inf = self.fp_inf(ty);
                let is_inf = self.fcmp(FloatPredicate::Oeq, abs, inf);
                let w = self.m.types.float_width(ty).unwrap_or(64);
                let it = self.m.types.int(w);
                let bits = self.cast(CastOp::BitCast, x, it);
                let zero = V::C(self.m.const_int(it, 0));
                let neg = self.icmp(IntPredicate::Slt, bits, zero);
                let rt = self.lty(q);
                let m1 = V::C(self.m.const_int(rt, -1));
                let p1 = V::C(self.m.const_int(rt, 1));
                let z = V::C(self.m.const_int(rt, 0));
                let sign = self.select(neg, m1, p1);
                self.select(is_inf, sign, z)
            }
            id::FPCLASSIFY => {
                return self.builtin_fpclassify(e, &args).map(|v| BuiltinCall::Emitted(Some(v)));
            }
            id::EXPECT | id::EXPECT_WITH_PROBABILITY => {
                // clang `BI__builtin_expect{,_with_probability}`: the value
                // and the expected value are emitted; at -O0 the value is
                // the result, above it `llvm.expect` (with the folded
                // probability as a `double`) of the two.
                let x = self.emit_scalar(a0)?;
                let Some(a1) = arg(1) else {
                    return err(self.loc_of(e), "__builtin_expect without two arguments");
                };
                let ev = self.emit_scalar(a1)?;
                let prob = match arg(2) {
                    Some(p) if b == id::EXPECT_WITH_PROBABILITY => {
                        let Some(c) = self.folds.get(p) else {
                            return err(
                                self.loc_of(e),
                                "__builtin_expect_with_probability probability is not a constant",
                            );
                        };
                        let pq = ast.exprs.get(p).ty();
                        Some(V::C(self.const_scalar_value(c, pq)?))
                    }
                    _ => None,
                };
                if self.opts.optimize() {
                    let ty = self.f.fir.ty(&self.m, x);
                    if let Some(p) = prob {
                        let f = kiln_llvm::intrinsics::expect_with_probability(&mut self.m, ty);
                        self.intrinsic_call(f, vec![x, ev, p])
                    } else {
                        let f = kiln_llvm::intrinsics::expect(&mut self.m, ty);
                        self.intrinsic_call(f, vec![x, ev])
                    }
                } else {
                    x
                }
            }
            // WP-282: `llvm.powi.T.i32` (clang `Builtin::BI__builtin_powi`).
            id::POWI | id::POWIF | id::POWIL => {
                let Some(n) = arg(1) else {
                    return err(self.loc_of(e), "__builtin_powi without two arguments");
                };
                let x = self.emit_scalar(a0)?;
                let nv = self.emit_scalar(n)?;
                let ty = self.f.fir.ty(&self.m, x);
                let it = self.f.fir.ty(&self.m, nv);
                let f = kiln_llvm::intrinsics::powi(&mut self.m, ty, it);
                self.intrinsic_call(f, vec![x, nv])
            }
            id::OBJECT_SIZE | id::DYNAMIC_OBJECT_SIZE => {
                return self
                    .builtin_object_size(b, e, &args)
                    .map(|v| BuiltinCall::Emitted(Some(v)));
            }
            id::ASSUME_ALIGNED => self.builtin_assume_aligned(e, &args)?,
            id::ASSUME => {
                // An assumption with possible side effects is never
                // evaluated (sema warned, WP-805).
                if self.ast.exprs.get(e).has(kiln_ast::expr::ExprFlags::ASSUME_IGNORED) {
                    return Ok(BuiltinCall::Emitted(None));
                }
                let c = self.emit_scalar(a0)?;
                let f = kiln_llvm::intrinsics::assume(&mut self.m);
                self.intrinsic_call(f, vec![c]);
                return Ok(BuiltinCall::Emitted(None));
            }
            // WP-726: clang `BI__builtin_assume_separate_storage`:
            // `llvm.assume(i1 true) ["separate_storage"(ptr a, ptr b)]`.
            id::ASSUME_SEPARATE_STORAGE => {
                let Some(a1) = arg(1) else {
                    return err(
                        self.loc_of(e),
                        "__builtin_assume_separate_storage without two arguments",
                    );
                };
                let pa = self.emit_scalar(a0)?;
                let pb = self.emit_scalar(a1)?;
                if self.f.fir.cur().is_some() {
                    let f = kiln_llvm::intrinsics::assume(&mut self.m);
                    let fn_ty = self.m.func(f).ty();
                    let cond = self.m.const_bool(true);
                    self.f.fir.push(
                        Op::AssumeBundle {
                            fn_ty,
                            callee: Value::func(f),
                            cond,
                            tag: "separate_storage",
                            args: vec![pa, pb],
                        },
                        LTy::VOID,
                    );
                }
                return Ok(BuiltinCall::Emitted(None));
            }
            id::VA_START | id::VA_END | id::VA_COPY => {
                // clang `EmitVAStartEnd` / `__builtin_va_copy`: the
                // `llvm.va_*` intrinsics on the `va_list`'s address (the
                // second `va_start` operand is not evaluated).
                let mut ops = vec![self.va_list_ref(a0)?];
                let f = match b {
                    id::VA_START => kiln_llvm::intrinsics::va_start(&mut self.m),
                    id::VA_END => kiln_llvm::intrinsics::va_end(&mut self.m),
                    _ => {
                        let Some(src) = arg(1) else {
                            return err(self.loc_of(e), "__builtin_va_copy without two arguments");
                        };
                        ops.push(self.va_list_ref(src)?);
                        kiln_llvm::intrinsics::va_copy(&mut self.m)
                    }
                };
                self.intrinsic_call(f, ops);
                return Ok(BuiltinCall::Emitted(None));
            }
            // WP-610: clang `BI__builtin_prefetch`: `llvm.prefetch(addr,
            // rw, locality, 1)`, `rw` 0 and `locality` 3 by default, the
            // trailing 1 the data cache. Sema checked both are constants.
            id::PREFETCH => {
                let addr = self.emit_scalar(a0)?;
                let mut ops = vec![addr];
                for (i, dflt) in [(1, 0), (2, 3)] {
                    let v = if let Some(x) = arg(i) {
                        if let Some(c) = self.fold_int(x) {
                            V::C(self.m.const_int(LTy::I32, c))
                        } else {
                            let v = self.emit_scalar(x)?;
                            self.int_cast(v, LTy::I32, true)
                        }
                    } else {
                        V::C(self.m.const_int(LTy::I32, dflt))
                    };
                    ops.push(v);
                }
                ops.push(V::C(self.m.const_int(LTy::I32, 1)));
                let f = kiln_llvm::intrinsics::prefetch(&mut self.m);
                self.intrinsic_call(f, ops);
                return Ok(BuiltinCall::Emitted(None));
            }
            // WP-630: clang `EmitNontemporalStore` / `EmitNontemporalLoad`:
            // an access through the pointer with its known alignment,
            // marked `!nontemporal !{i32 1}`.
            id::NONTEMPORAL_STORE => {
                let Some(p) = arg(1) else {
                    return err(self.loc_of(e), "__builtin_nontemporal_store needs two arguments");
                };
                let v = self.emit_scalar(a0)?;
                let (ptr, align) = self.ptr_align(p)?;
                let st = self.store(v, ptr, align, false);
                let vq = ast.exprs.get(a0).ty();
                self.tbaa_mark(st, vq, crate::tbaa::Tb::Type);
                self.mark_nontemporal(st)?;
                return Ok(BuiltinCall::Emitted(None));
            }
            id::NONTEMPORAL_LOAD => {
                let (ptr, align) = self.ptr_align(a0)?;
                let ty = self.lty(q);
                let v = self.load(ty, ptr, align, false);
                self.tbaa_mark(v, q, crate::tbaa::Tb::Type);
                self.mark_nontemporal(v)?;
                v
            }
            // clang: `freeze poison`, the operand not evaluated.
            id::NONDETERMINISTIC_VALUE => {
                let ty = self.lty(q);
                let p = V::C(self.m.const_poison(ty));
                self.f.fir.push(Op::Freeze { v: p }, ty)
            }
            // clang `EmitTrapCall`: no `unreachable` follows. The
            // `__builtin_verbose_trap` category and message only reach the
            // debug info (WP-282).
            // clang `BI__builtin_debugtrap`: `llvm.debugtrap()`.
            id::DEBUGTRAP => {
                let f = kiln_llvm::intrinsics::debugtrap(&mut self.m);
                self.intrinsic_call(f, vec![]);
                return Ok(BuiltinCall::Emitted(None));
            }
            // P1494R5 (WP-757): an opaque memory barrier
            // ([`Cg::emit_observable_checkpoint`]).
            id::OBSERVABLE_CHECKPOINT => {
                self.emit_observable_checkpoint()?;
                return Ok(BuiltinCall::Emitted(None));
            }
            // P3726R2 (WP-773): GCC folds `__builtin_start_lifetime(p)` to
            // `(void)p`: the pointer is evaluated, nothing else is emitted.
            id::START_LIFETIME => {
                self.emit_ignored(a0)?;
                return Ok(BuiltinCall::Emitted(None));
            }
            id::TRAP | id::VERBOSE_TRAP => {
                let f = kiln_llvm::intrinsics::trap(&mut self.m);
                self.intrinsic_call(f, vec![]);
                return Ok(BuiltinCall::Emitted(None));
            }
            id::MEMCPY | id::MEMMOVE | id::MEMPCPY | id::MEMCPY_INLINE => {
                let (Some(d), Some(s), Some(n)) = (arg(0), arg(1), arg(2)) else {
                    return err(self.loc_of(e), "memory builtin without three arguments");
                };
                let (dv, da) = self.ptr_align(d)?;
                let (sv, sa) = self.ptr_align(s)?;
                let nv = self.emit_scalar(n)?;
                let f = if b == id::MEMMOVE {
                    kiln_llvm::intrinsics::memmove(&mut self.m, LTy::I64)
                } else if b == id::MEMCPY_INLINE {
                    // WP-726: clang `BI__builtin_memcpy_inline`.
                    kiln_llvm::intrinsics::memcpy_inline(&mut self.m, LTy::I64)
                } else {
                    kiln_llvm::intrinsics::memcpy(&mut self.m, LTy::I64)
                };
                let fn_ty = self.m.func(f).ty();
                let vb = self.m.const_bool(false);
                let pa = self.m.attr_set(&[Attr::Align(da)]);
                let pb = self.m.attr_set(&[Attr::Align(sa)]);
                let none = self.m.attr_set(&[]);
                let opts = CallOpts {
                    attrs: AttrList { params: vec![pa, pb, none, none], ..AttrList::default() },
                    ..CallOpts::default()
                };
                let c = self.f.fir.push(
                    Op::Call {
                        fn_ty,
                        callee: V::C(Value::func(f)),
                        args: vec![dv, sv, nv, V::C(vb)],
                        opts: Box::new(opts),
                    },
                    LTy::VOID,
                );
                self.f.calls.push((Fir::last_inst(c), Some(f), true));
                if b == id::MEMCPY_INLINE {
                    return Ok(BuiltinCall::Emitted(None));
                }
                if b == id::MEMPCPY {
                    // clang: the end of the destination.
                    self.gep(LTy::I8, dv, vec![nv], true)
                } else {
                    dv
                }
            }
            // WP-150: clang `EmitBuiltinExpr` for `memset` and the GCC
            // integer builtins.
            id::MEMSET => {
                let (Some(d), Some(c), Some(n)) = (arg(0), arg(1), arg(2)) else {
                    return err(self.loc_of(e), "memory builtin without three arguments");
                };
                let (dv, da) = self.ptr_align(d)?;
                let cv = self.emit_scalar(c)?;
                let cv = self.int_cast(cv, LTy::I8, false);
                let nv = self.emit_scalar(n)?;
                let f = kiln_llvm::intrinsics::memset(&mut self.m, LTy::I64);
                let fn_ty = self.m.func(f).ty();
                let vb = self.m.const_bool(false);
                let pa = self.m.attr_set(&[Attr::Align(da)]);
                let none = self.m.attr_set(&[]);
                let opts = CallOpts {
                    attrs: AttrList { params: vec![pa, none, none, none], ..AttrList::default() },
                    ..CallOpts::default()
                };
                let c = self.f.fir.push(
                    Op::Call {
                        fn_ty,
                        callee: V::C(Value::func(f)),
                        args: vec![dv, cv, nv, V::C(vb)],
                        opts: Box::new(opts),
                    },
                    LTy::VOID,
                );
                self.f.calls.push((Fir::last_inst(c), Some(f), true));
                dv
            }
            id::FFS | id::FFSL | id::FFSLL => {
                // `x ? cttz(x, true) + 1 : 0`.
                let x = self.emit_scalar(a0)?;
                let ty = self.f.fir.ty(&self.m, x);
                let f = kiln_llvm::intrinsics::cttz(&mut self.m, ty);
                let t = V::C(self.m.const_bool(true));
                let tz = self.intrinsic_call(f, vec![x, t]);
                let one = V::C(self.m.const_int(ty, 1));
                let r = self.bin(BinOp::Add, Flags::NONE, tz, one);
                let zero = V::C(self.m.const_int(ty, 0));
                let is_zero = self.icmp(IntPredicate::Eq, x, zero);
                let r = self.select(is_zero, zero, r);
                let rt = self.lty(q);
                self.int_cast(r, rt, false)
            }
            id::PARITY | id::PARITYL | id::PARITYLL => {
                let x = self.emit_scalar(a0)?;
                let ty = self.f.fir.ty(&self.m, x);
                let f = kiln_llvm::intrinsics::ctpop(&mut self.m, ty);
                let r = self.intrinsic_call(f, vec![x]);
                let one = V::C(self.m.const_int(ty, 1));
                let r = self.bin(BinOp::And, Flags::NONE, r, one);
                let rt = self.lty(q);
                self.int_cast(r, rt, false)
            }
            id::CLRSB | id::CLRSBL | id::CLRSBLL => {
                // `ctlz(x < 0 ? ~x : x, false) - 1`.
                let x = self.emit_scalar(a0)?;
                let ty = self.f.fir.ty(&self.m, x);
                let zero = V::C(self.m.const_int(ty, 0));
                let neg = self.icmp(IntPredicate::Slt, x, zero);
                let all = V::C(self.m.const_int(ty, -1));
                let inv = self.bin(BinOp::Xor, Flags::NONE, x, all);
                let v = self.select(neg, inv, x);
                let f = kiln_llvm::intrinsics::ctlz(&mut self.m, ty);
                let fl = V::C(self.m.const_bool(false));
                let lz = self.intrinsic_call(f, vec![v, fl]);
                let one = V::C(self.m.const_int(ty, 1));
                let r = self.bin(BinOp::Sub, Flags::NONE, lz, one);
                let rt = self.lty(q);
                self.int_cast(r, rt, false)
            }
            id::BITREVERSE8 | id::BITREVERSE16 | id::BITREVERSE32 | id::BITREVERSE64 => {
                let x = self.emit_scalar(a0)?;
                let ty = self.f.fir.ty(&self.m, x);
                let f = kiln_llvm::intrinsics::fp_op(&mut self.m, "llvm.bitreverse", ty, 1);
                self.intrinsic_call(f, vec![x])
            }
            id::ROTATELEFT8
            | id::ROTATELEFT16
            | id::ROTATELEFT32
            | id::ROTATELEFT64
            | id::ROTATERIGHT8
            | id::ROTATERIGHT16
            | id::ROTATERIGHT32
            | id::ROTATERIGHT64 => {
                // `llvm.fshl(x, x, n)` / `llvm.fshr(x, x, n)`.
                let Some(n) = arg(1) else {
                    return err(self.loc_of(e), "rotate builtin without two arguments");
                };
                let x = self.emit_scalar(a0)?;
                let nv = self.emit_scalar(n)?;
                let ty = self.f.fir.ty(&self.m, x);
                let left = matches!(
                    b,
                    id::ROTATELEFT8 | id::ROTATELEFT16 | id::ROTATELEFT32 | id::ROTATELEFT64
                );
                let name = if left { "llvm.fshl" } else { "llvm.fshr" };
                let f = kiln_llvm::intrinsics::fp_op(&mut self.m, name, ty, 3);
                self.intrinsic_call(f, vec![x, x, nv])
            }
            // [ptr.launder]: the pointer itself (no `llvm.launder.invariant.group`
            // without `-fstrict-vtable-pointers`).
            id::LAUNDER => self.emit_scalar(a0)?,
            // clang `BI__builtin_unpredictable`: the operand; the branch
            // or `switch` on the call carries the `!unpredictable` hint
            // (`Cg::mark_unpredictable`).
            id::UNPREDICTABLE => self.emit_scalar(a0)?,
            id::OPERATOR_NEW | id::OPERATOR_DELETE => {
                return self.builtin_new_delete(b, e, &args).map(BuiltinCall::Emitted);
            }
            id::UNREACHABLE => {
                // clang `EmitUnreachable`, then an insertion point.
                self.f.fir.push(Op::Unreachable, LTy::VOID);
                self.f.fir.set_cur(None);
                let cont = self.f.fir.create_block();
                self.emit_block(cont, false);
                return Ok(BuiltinCall::Emitted(None));
            }
            id::atomic::CLEAR
            | id::atomic::IS_LOCK_FREE
            | id::atomic::SIGNAL_FENCE
            | id::atomic::TEST_AND_SET
            | id::atomic::THREAD_FENCE
            | id::atomic::C11_IS_LOCK_FREE
            | id::atomic::C11_SIGNAL_FENCE
            | id::atomic::C11_THREAD_FENCE => {
                return self.emit_atomic_builtin(b, e, &args).map(BuiltinCall::Emitted);
            }
            _ if id::sync::op(b).is_some()
                || matches!(
                    b,
                    id::sync::SYNCHRONIZE
                        | id::sync::FETCH_AND_MAX
                        | id::sync::FETCH_AND_MIN
                        | id::sync::FETCH_AND_UMAX
                        | id::sync::FETCH_AND_UMIN
                ) =>
            {
                return self.emit_sync_builtin(b, e, &args).map(BuiltinCall::Emitted);
            }
            _ if id::checked_op(b).is_some() => self.emit_checked(b, e, &args)?,
            _ if id::carry_op(b).is_some() => self.emit_carry(b, e, &args)?,
            id::ALLOCA | id::ALLOCA_WITH_ALIGN => self.emit_alloca(b, e, &args)?,
            _ if lib_name(b).is_some() => return Ok(BuiltinCall::Library),
            _ => {
                let name = id::info(b).map_or("?", |i| i.name);
                return err(
                    self.loc_of(e),
                    format!("code generation for {name} is not supported yet"),
                );
            }
        };
        Ok(BuiltinCall::Emitted(Some(v)))
    }

    /// `__builtin_nextafterf16`/`f16b` (WP-365): there is no library entry
    /// point (clang 19 has neither builtin; GCC calls an undefined
    /// `nextafterf16`), so the step is inline on the 16 bits, as
    /// libstdc++'s run-time `nextafter(_Float16, _Float16)`: a NaN operand
    /// gives that NaN quieted (`x`'s first), equal operands (or two zeros)
    /// `y`, a zero `x` the smallest subnormal with `y`'s sign, else the
    /// bits of `x` plus one when moving away from zero and minus one
    /// toward it. After one bitcast of each operand everything is integer
    /// arithmetic: LLVM 19's x86-64 backend, which promotes `bfloat` to
    /// `float`, miscompiles a `bitcast bfloat to i16` whose operand also
    /// feeds an `fpext`/`fcmp` (it loses the sign bit at -O2), so no
    /// floating-point operation touches the operands.
    fn nextafter16(&mut self, x: V, y: V) -> V {
        let ty = self.f.fir.ty(&self.m, x);
        let (inf, quiet) = if ty == LTy::BFLOAT { (0x7f80, 0x40) } else { (0x7c00, 0x200) };
        let i16 = LTy::I16;
        let (hx, hy) = (self.cast(CastOp::BitCast, x, i16), self.cast(CastOp::BitCast, y, i16));
        let c = |s: &mut Self, v: i128| V::C(s.m.const_int(i16, v));
        let (abs_mask, sign_mask, one, m1, zero, inf, quiet) = (
            c(self, 0x7fff),
            c(self, -0x8000),
            c(self, 1),
            c(self, -1),
            c(self, 0),
            c(self, inf),
            c(self, quiet),
        );
        let ax = self.bin(BinOp::And, Flags::NONE, hx, abs_mask);
        let ay = self.bin(BinOp::And, Flags::NONE, hy, abs_mask);
        // Sign-magnitude to two's-complement order: a negative encoding
        // has its magnitude bits flipped, so `kx < ky` iff `x < y`.
        let key = |s: &mut Self, h: V, neg: V| {
            let flip = s.bin(BinOp::Xor, Flags::NONE, h, abs_mask);
            s.select(neg, flip, h)
        };
        let neg_x = self.icmp(IntPredicate::Slt, hx, zero);
        let neg_y = self.icmp(IntPredicate::Slt, hy, zero);
        let kx = key(self, hx, neg_x);
        let ky = key(self, hy, neg_y);
        let lt = self.icmp(IntPredicate::Slt, kx, ky);
        let away = self.bin(BinOp::Xor, Flags::NONE, lt, neg_x);
        let d = self.select(away, one, m1);
        let step = self.bin(BinOp::Add, Flags::NONE, hx, d);
        let x_zero = self.icmp(IntPredicate::Eq, ax, zero);
        let sy = self.bin(BinOp::And, Flags::NONE, hy, sign_mask);
        let tiny = self.bin(BinOp::Or, Flags::NONE, sy, one);
        let bits = self.select(x_zero, tiny, step);
        let same = self.icmp(IntPredicate::Eq, hx, hy);
        let both = self.bin(BinOp::Or, Flags::NONE, ax, ay);
        let zeros = self.icmp(IntPredicate::Eq, both, zero);
        let eq = self.bin(BinOp::Or, Flags::NONE, same, zeros);
        let bits = self.select(eq, hy, bits);
        let nan_x = self.icmp(IntPredicate::Ugt, ax, inf);
        let nan_y = self.icmp(IntPredicate::Ugt, ay, inf);
        let qx = self.bin(BinOp::Or, Flags::NONE, hx, quiet);
        let qy = self.bin(BinOp::Or, Flags::NONE, hy, quiet);
        let qn = self.select(nan_x, qx, qy);
        let nan = self.bin(BinOp::Or, Flags::NONE, nan_x, nan_y);
        let bits = self.select(nan, qn, bits);
        self.cast(CastOp::BitCast, bits, ty)
    }

    /// clang `emitBuiltinObjectSize` for a pointer the fold prepass could
    /// not evaluate (WP-282): type 3 is the default 0 without evaluating
    /// the pointer; otherwise `llvm.objectsize(p, min, nullunknown=true,
    /// dynamic)` with `min` the type's bit 1.
    fn builtin_object_size(&mut self, b: u32, e: ExprId, args: &[ExprId]) -> R<V> {
        let rt = self.lty(self.ast.exprs.get(e).ty());
        let (Some(&p), Some(t)) = (args.first(), args.get(1).and_then(|&t| self.fold_int(t)))
        else {
            return err(self.loc_of(e), "__builtin_object_size without a constant type");
        };
        if t == 3 {
            return Ok(V::C(self.m.const_int(rt, 0)));
        }
        let pv = self.emit_scalar(p)?;
        let f = kiln_llvm::intrinsics::objectsize(&mut self.m, rt);
        let min = V::C(self.m.const_bool(t & 2 != 0));
        let null_unknown = V::C(self.m.const_bool(true));
        let dynamic = V::C(self.m.const_bool(b == id::DYNAMIC_OBJECT_SIZE));
        Ok(self.intrinsic_call(f, vec![pv, min, null_unknown, dynamic]))
    }

    /// clang `Builtin::BI__builtin_assume_aligned` (WP-282): the pointer,
    /// after `llvm.assume(i1 true) ["align"(ptr p, i64 a[, i64 off])]`
    /// with the alignment clamped to 2^32 (clang `MaximumAlignment`).
    fn builtin_assume_aligned(&mut self, e: ExprId, args: &[ExprId]) -> R<V> {
        let (Some(&p), Some(a)) = (args.first(), args.get(1).and_then(|&a| self.fold_int(a)))
        else {
            return err(self.loc_of(e), "__builtin_assume_aligned without a constant alignment");
        };
        let pv = self.emit_scalar(p)?;
        let off = match args.get(2) {
            Some(&o) => Some(self.emit_scalar(o)?),
            None => None,
        };
        let a = a.clamp(1, 1 << 32);
        let mut bundle = vec![pv, V::C(self.m.const_int(LTy::I64, a))];
        if let Some(o) = off {
            bundle.push(self.int_cast(o, LTy::I64, false));
        }
        if self.f.fir.cur().is_some() {
            let f = kiln_llvm::intrinsics::assume(&mut self.m);
            let fn_ty = self.m.func(f).ty();
            let cond = self.m.const_bool(true);
            self.f.fir.push(
                Op::AssumeBundle {
                    fn_ty,
                    callee: Value::func(f),
                    cond,
                    tag: "align",
                    args: bundle,
                },
                LTy::VOID,
            );
        }
        Ok(pv)
    }

    /// clang `EmitFAbs`: `llvm.fabs` with a `memory(none)` call site.
    fn fabs_no_mem(&mut self, x: V, ty: LTy) -> V {
        let f = kiln_llvm::intrinsics::fabs(&mut self.m, ty);
        let fa = self.m.attr_set(&[Attr::Memory(MemoryEffects::all(ModRef::None))]);
        let opts =
            CallOpts { attrs: AttrList { func: fa, ..AttrList::default() }, ..CallOpts::default() };
        let fn_ty = self.m.func(f).ty();
        let r = self.f.fir.push(
            Op::Call { fn_ty, callee: V::C(Value::func(f)), args: vec![x], opts: Box::new(opts) },
            ty,
        );
        self.f.calls.push((Fir::last_inst(r), Some(f), true));
        r
    }

    /// Positive infinity of floating-point type `ty`.
    fn fp_inf(&mut self, ty: LTy) -> V {
        V::C(self.m.const_float(ty, f64::INFINITY))
    }

    /// clang `EmitBuiltinExpr` for `__builtin_fpclassify(nan, inf, normal,
    /// subnormal, zero, x)`: a compare chain into a `phi`, the end block
    /// laid out right after the current one (`createBasicBlock(.., CurFn)`).
    fn builtin_fpclassify(&mut self, e: ExprId, args: &[ExprId]) -> R<V> {
        if args.len() != 6 {
            return err(self.loc_of(e), "__builtin_fpclassify without six arguments");
        }
        let x = self.emit_scalar(args[5])?;
        let ty = self.f.fir.ty(&self.m, x);
        let rq = self.ast.exprs.get(args[0]).ty();
        let rt = self.lty(rq);
        let Some(begin) = self.f.fir.cur() else {
            return err(self.loc_of(e), "__builtin_fpclassify without an insertion point");
        };
        let end = self.f.fir.create_block();
        self.f.fir.place_at_end(end);
        let phi = self.f.fir.push_in(end, Op::Phi { ty: rt, inc: vec![] }, rt);
        let phi_i = Fir::last_inst(phi);
        // x == 0 → zero
        let zero = V::C(self.m.const_float(ty, 0.0));
        let is_zero = self.fcmp(FloatPredicate::Oeq, x, zero);
        let zv = self.emit_scalar(args[4])?;
        let not_zero = self.f.fir.create_block();
        self.f.fir.place_at_end(not_zero);
        self.cond_br(is_zero, end, not_zero);
        let from = self.f.fir.cur().unwrap_or(begin);
        self.f.fir.add_incoming(phi_i, zv, from);
        // x != x → nan
        self.f.fir.set_cur(Some(not_zero));
        let is_nan = self.fcmp(FloatPredicate::Uno, x, x);
        let nv = self.emit_scalar(args[0])?;
        let not_nan = self.f.fir.create_block();
        self.f.fir.place_at_end(not_nan);
        self.cond_br(is_nan, end, not_nan);
        let from = self.f.fir.cur().unwrap_or(not_zero);
        self.f.fir.add_incoming(phi_i, nv, from);
        // |x| == inf → infinite
        self.f.fir.set_cur(Some(not_nan));
        let abs = self.fabs_no_mem(x, ty);
        let inf = self.fp_inf(ty);
        let is_inf = self.fcmp(FloatPredicate::Oeq, abs, inf);
        let iv = self.emit_scalar(args[1])?;
        let not_inf = self.f.fir.create_block();
        self.f.fir.place_at_end(not_inf);
        self.cond_br(is_inf, end, not_inf);
        let from = self.f.fir.cur().unwrap_or(not_nan);
        self.f.fir.add_incoming(phi_i, iv, from);
        // |x| >= smallest normal → normal, else subnormal
        self.f.fir.set_cur(Some(not_inf));
        let w = self.m.types.float_width(ty).unwrap_or(64);
        let min_bits: u128 = match w {
            32 => 0x0080_0000,
            16 => 0x0400,
            80 => 0x0001_8000_0000_0000_0000,
            128 => 1u128 << 112,
            _ => 0x0010_0000_0000_0000,
        };
        let min = V::C(self.m.const_float_bits(ty, min_bits));
        let is_normal = self.fcmp(FloatPredicate::Uge, abs, min);
        let normal = self.emit_scalar(args[2])?;
        let sub = self.emit_scalar(args[3])?;
        let r = self.select(is_normal, normal, sub);
        let from = self.f.fir.cur().unwrap_or(not_inf);
        self.br(end);
        self.f.fir.add_incoming(phi_i, r, from);
        self.f.fir.set_cur(Some(end));
        Ok(phi)
    }

    /// A call of intrinsic `f` with no call-site attributes.
    pub(crate) fn intrinsic_call(&mut self, f: FuncId, args: Vec<V>) -> V {
        let fty = self.m.func(f).ty();
        let rty = self.m.types.function_info(fty).map_or(LTy::VOID, |i| i.0);
        self.intrinsic_call_ty(f, args, rty)
    }

    pub(crate) fn intrinsic_call_ty(&mut self, f: FuncId, args: Vec<V>, rty: LTy) -> V {
        let fn_ty = self.m.func(f).ty();
        let r = self.f.fir.push(
            Op::Call { fn_ty, callee: V::C(Value::func(f)), args, opts: Box::default() },
            rty,
        );
        self.f.calls.push((Fir::last_inst(r), Some(f), true));
        r
    }

    /// The pointer value of `e` and the alignment it is known to have
    /// (clang `EmitPointerWithAlignment`).
    pub(crate) fn ptr_align(&mut self, e: ExprId) -> R<(V, u64)> {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        if n.kind() == ExprKind::ParenExpr {
            return self.ptr_align(ast.exprs.operand(e));
        }
        if n.kind().is_cast() {
            let s = ast.exprs.operand(e);
            match ast.exprs.cast_kind(e) {
                CastKind::NoOp | CastKind::BitCast => {
                    let sq = ast.exprs.get(s).ty();
                    if let Some(p) = ast.types.pointee(sq)
                        && ast.types.is_pointer(sq)
                        && !ast.types.is_void(p)
                    {
                        let (v, a) = self.ptr_align(s)?;
                        if n.kind() == ExprKind::ImplicitCastExpr || self.decl_aligned(s) {
                            return Ok((v, a));
                        }
                        // An explicit cast honors the target type's
                        // alignment unless the source is a declaration.
                        let a = self.pointee_align(n.ty());
                        return Ok((v, a));
                    }
                }
                CastKind::ArrayToPointerDecay => {
                    let lv = self.emit_lvalue(s)?;
                    let v = self.decay(lv.addr, lv.q);
                    return Ok((v, lv.addr.align));
                }
                _ => {}
            }
        }
        if n.kind() == ExprKind::UnaryOperator && ast.exprs.unary_op(e) == UnaryOp::AddrOf {
            let lv = self.emit_lvalue(ast.exprs.operand(e))?;
            return Ok((lv.addr.v, lv.addr.align));
        }
        let v = self.emit_scalar(e)?;
        let a = self.pointee_align(n.ty());
        Ok((v, a))
    }

    /// Whether the pointer `e` (through implicit casts) has its alignment
    /// from a declaration (`AlignmentSource::Decl`).
    fn decl_aligned(&self, e: ExprId) -> bool {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        match n.kind() {
            ExprKind::ParenExpr => self.decl_aligned(ast.exprs.operand(e)),
            ExprKind::ImplicitCastExpr => match ast.exprs.cast_kind(e) {
                CastKind::NoOp | CastKind::BitCast => self.decl_aligned(ast.exprs.operand(e)),
                CastKind::ArrayToPointerDecay => self.lvalue_from_decl(ast.exprs.operand(e)),
                _ => false,
            },
            ExprKind::UnaryOperator if ast.exprs.unary_op(e) == UnaryOp::AddrOf => {
                self.lvalue_from_decl(ast.exprs.operand(e))
            }
            _ => false,
        }
    }

    /// clang `getNaturalPointeeTypeAlignment`: an incomplete or `void`
    /// pointee has alignment 1.
    fn pointee_align(&mut self, pq: QualType) -> u64 {
        let ast = self.ast;
        match ast.types.pointee(pq) {
            Some(p)
                if !ast.types.is_void(p) && !ast.types.is_function(p) && is_complete(ast, p) =>
            {
                self.size_align(p).1
            }
            _ => 1,
        }
    }

    /// The library function a library builtin `d` calls (clang
    /// `getBuiltinLibFunction`): declared with the builtin's type and
    /// attributes under the library name.
    pub(crate) fn lib_fn(&mut self, d: DeclId, b: u32) -> R<(FuncId, Vec<Attr>)> {
        let ast = self.ast;
        let Some(name) = lib_name(b) else {
            return err(Some(ast.decls.get(d).loc()), "not a library builtin");
        };
        let Some(fty) = ast.decls.get(d).ty() else {
            return err(None, "builtin without a type");
        };
        let sig = self.sig(fty, true);
        let mut extra = Vec::new();
        lib_attrs(b, sig.passes_indirect(), &mut extra);
        if let Some(f) = self.m.get_function(name) {
            return Ok((f, extra));
        }
        let f = match self.m.add_function(name, sig.fn_ty) {
            Ok(f) => f,
            Err(x) => {
                return err(Some(ast.decls.get(d).loc()), format!("cannot declare {name}: {x:?}"));
            }
        };
        let (ret, params) = self.sig_attrs(&sig, sig.params.len(), false);
        let mut fa = Vec::new();
        if sig.nothrow {
            fa.push(Attr::Flag(AttrKind::NoUnwind));
        }
        fa.extend(extra.iter().cloned());
        self.target_attrs(&mut fa);
        let fa = self.m.attr_set(&fa);
        let func = self.m.func_mut(f);
        func.attrs.func = fa;
        func.attrs.ret = ret;
        func.attrs.params = params;
        self.nounwind.insert(f, sig.nothrow);
        Ok((f, extra))
    }

    /// clang `EmitBuiltinNewDeleteCall`: a call of the global replaceable
    /// `operator new` / `operator delete` whose type sema gave the callee.
    fn builtin_new_delete(&mut self, b: u32, e: ExprId, args: &[ExprId]) -> R<Option<V>> {
        let ast = self.ast;
        let callee = ast.exprs.callee(e);
        let fty = ast.exprs.get(callee).ty();
        let fty = ast.types.pointee(fty).unwrap_or(fty);
        let op = if b == id::OPERATOR_DELETE {
            OverloadedOperator::Delete
        } else {
            OverloadedOperator::New
        };
        let canon = ast.types.canonical(fty);
        let found = ast.lookup_local(ast.tu, DeclName::operator(op)).iter().find(|&f| {
            ast.decls.get(f).kind() == DeclKind::Function
                && ast.decls.get(f).ty().is_some_and(|t| ast.types.canonical(t) == canon)
        });
        let Some(f) = found else {
            return err(self.loc_of(e), "no global allocation function for the builtin");
        };
        let params: Vec<QualType> =
            ast.types.proto(fty).map(|p| ast.types.params(p).to_vec()).unwrap_or_default();
        let mut vals = Vec::with_capacity(args.len());
        let mut tys = Vec::with_capacity(args.len());
        for (i, &a) in args.iter().enumerate() {
            let pq = params.get(i).copied().unwrap_or(ast.exprs.get(a).ty());
            tys.push(ast.exprs.get(a).ty());
            vals.push(if ast.types.is_reference(pq) {
                CallArg::Scalar(self.emit_lvalue(a)?.addr.v)
            } else {
                CallArg::Scalar(self.emit_scalar(a)?)
            });
        }
        let loc = self.loc_of(e);
        let r = self.emit_new_delete_call(f, &vals, &tys, loc)?;
        Ok((b == id::OPERATOR_NEW).then_some(r))
    }
}
