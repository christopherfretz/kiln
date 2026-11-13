//! Attributes: the 16-byte [`AttrNode`] (spec/ast.md §7).
#![warn(missing_docs)]

use kiln_source::Loc;

use crate::ids::{AttrId, DeclId, ListId};
use crate::mem::{HeapSize, MemReport, MemRow};
use crate::slot::ColdMap;

/// An attribute kind. A newtype over `u16` so the list grows without
/// renumbering; unknown attributes are diagnosed and never stored.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kiln_snap::Snap)]
pub struct AttrKind(pub u16);

#[allow(missing_docs, reason = "each constant is named after the attribute it denotes")]
impl AttrKind {
    // standard ([dcl.attr])
    pub const ALIGNED: AttrKind = AttrKind(1);
    pub const ASSUME: AttrKind = AttrKind(2);
    pub const CARRIES_DEPENDENCY: AttrKind = AttrKind(3);
    pub const DEPRECATED: AttrKind = AttrKind(4);
    pub const FALLTHROUGH: AttrKind = AttrKind(5);
    pub const INDETERMINATE: AttrKind = AttrKind(6);
    pub const LIKELY: AttrKind = AttrKind(7);
    pub const UNLIKELY: AttrKind = AttrKind(8);
    pub const MAYBE_UNUSED: AttrKind = AttrKind(9);
    pub const NODISCARD: AttrKind = AttrKind(10);
    pub const NORETURN: AttrKind = AttrKind(11);
    pub const NO_UNIQUE_ADDRESS: AttrKind = AttrKind(12);
    /// P3394 annotation `[[=e]]`: `a` = the operand `ExprId`. Only between
    /// parsing and sema: sema moves each into `Ast::annots`
    /// (spec/annotations.md), so no declaration keeps one.
    pub const ANNOTATION: AttrKind = AttrKind(13);
    // GNU / vendor
    pub const ALWAYS_INLINE: AttrKind = AttrKind(64);
    pub const NOINLINE: AttrKind = AttrKind(65);
    pub const PACKED: AttrKind = AttrKind(66);
    pub const USED: AttrKind = AttrKind(67);
    pub const UNUSED: AttrKind = AttrKind(68);
    pub const WEAK: AttrKind = AttrKind(69);
    /// GNU `visibility("v")`: `a` = 0 `default`, 1 `hidden` (also
    /// `internal`), 2 `protected` ([`AttrNode::visibility`]); the
    /// implicit flag marks one added by `#pragma GCC visibility push`
    /// (spec/codegen.md "Symbol visibility").
    pub const VISIBILITY: AttrKind = AttrKind(70);
    /// GNU `section("name")` on a function or a variable with static
    /// storage duration (clang `SectionAttr`): `a` = the name's
    /// `StringLitId`; codegen places the symbol in that section
    /// ([`crate::Ast::section_of`]).
    pub const SECTION: AttrKind = AttrKind(71);
    /// GNU `format(archetype, F, A)`: `a` = the archetype (0 `printf`),
    /// `b` = `F | A << 16`, the one-based format-string and first data
    /// argument indices (`A` = 0 for a `va_list`). Only the implicit
    /// attribute of the printf-family builtins is stored (WP-282).
    pub const FORMAT: AttrKind = AttrKind(72);
    pub const CONST: AttrKind = AttrKind(73);
    pub const PURE: AttrKind = AttrKind(74);
    pub const COLD: AttrKind = AttrKind(75);
    pub const NOTHROW: AttrKind = AttrKind(76);
    pub const ENABLE_IF: AttrKind = AttrKind(77);
    pub const DIAGNOSE_IF: AttrKind = AttrKind(78);
    /// `[[clang::trivial_abi]]` / `__attribute__((trivial_abi))` on a
    /// class (clang `TrivialABIAttr`): when it is in effect (the class
    /// bit `DefinitionBits::TRIVIAL_ABI`) the class is trivial for the
    /// purposes of calls, passed and returned in registers, and the
    /// callee destroys a by-value parameter (spec/abi.md §8.25; WP-460).
    pub const TRIVIAL_ABI: AttrKind = AttrKind(79);
    pub const PREFERRED_NAME: AttrKind = AttrKind(80);
    pub const EXCLUDE_FROM_EXPLICIT_INSTANTIATION: AttrKind = AttrKind(81);
    pub const ABI_TAG: AttrKind = AttrKind(82);
    /// clang `internal_linkage` on a variable, function or class (clang
    /// `InternalLinkageAttr`): the entity, and a class's members, have
    /// internal linkage as if declared `static` (WP-807).
    pub const INTERNAL_LINKAGE: AttrKind = AttrKind(83);
    /// clang `lifetimebound` (WP-674): on a parameter; on a member
    /// function (written in its function type) with `a` = 1, for the
    /// implicit object parameter.
    pub const LIFETIMEBOUND: AttrKind = AttrKind(84);
    /// The `no_sanitize*` family (clang `NoSanitizeAttr`, WP-675): `a`
    /// and `b` hold the excluded set and the spelling
    /// (`kiln_ast::sanitize::no_sanitize_words`).
    pub const NO_SANITIZE: AttrKind = AttrKind(85);
    /// GNU `mode(M)` (clang `ModeAttr`), consumed into the declared type
    /// like the vector attributes: `a` = the mode's width in bits, `| 1
    /// << 8` for a floating-point mode, `| 2 << 8` for `KF` (IEEE quad),
    /// `| 3 << 8` for `IF` (IBM double-double), `| 1 << 11` for a complex
    /// mode (`SC`, `TC`, ..), `| (log2(N) + 1) << 12` for a vector mode
    /// `V<N>M`; `b` = the written `Symbol` (WP-799).
    pub const MODE: AttrKind = AttrKind(86);
    pub const VECTOR_SIZE: AttrKind = AttrKind(87);
    /// GNU asm label (`asm("name")` after a declarator): `a` = the
    /// label's `StringLitId`. Keyword syntax.
    pub const ASM_LABEL: AttrKind = AttrKind(88);
    /// The implicit attribute of a lazily declared builtin function
    /// (clang's `BuiltinAttr`): `a` = its builtin id
    /// ([`crate::decl::builtin`]). Always implicit.
    pub const BUILTIN: AttrKind = AttrKind(89);
    /// Clang `ext_vector_type(N)` on a typedef: `a` = the element-count
    /// `ExprId`. Like `VECTOR_SIZE`, it is consumed into the declared type
    /// (a `Vector` / `DependentVector`) and never attached to the decl.
    pub const EXT_VECTOR_TYPE: AttrKind = AttrKind(90);
    /// GNU `malloc` on a function returning a pointer (clang's
    /// `RestrictAttr`): the result is `noalias` (WP-170).
    pub const MALLOC: AttrKind = AttrKind(97);
    /// GNU `alloc_size(n[, m])` (clang `AllocSizeAttr`): `a` / `b` are
    /// `IntegerLiteral`s holding the one-based parameter indices n / m
    /// (`b` = 0 when absent) (WP-170).
    pub const ALLOC_SIZE: AttrKind = AttrKind(98);
    /// GNU `alloc_align(n)` (clang `AllocAlignAttr`): `a` is an
    /// `IntegerLiteral` holding the one-based parameter index (WP-170).
    pub const ALLOC_ALIGN: AttrKind = AttrKind(99);
    /// `clang::using_if_exists` on a using-declaration (WP-062).
    pub const USING_IF_EXISTS: AttrKind = AttrKind(100);
    /// GNU `hot` on a function (clang `HotAttr`): the declaration and its
    /// direct call sites are `hot` (WP-172).
    pub const HOT: AttrKind = AttrKind(101);
    /// `flag_enum` / `clang::flag_enum` on an enumeration (clang
    /// `FlagEnumAttr`): its values are bit flags, so a `case` label made
    /// of its single-bit enumerators' bits is in the enumeration (WP-230).
    pub const FLAG_ENUM: AttrKind = AttrKind(102);
    /// `enum_extensibility(open|closed)` on an enumeration (clang
    /// `EnumExtensibilityAttr`): `a` is 1 for `open` (values outside the
    /// enumerators are expected), 0 for `closed` (WP-230).
    pub const ENUM_EXTENSIBILITY: AttrKind = AttrKind(103);
    /// `clang::coro_only_destroy_when_complete` on a class (clang
    /// `CoroOnlyDestroyWhenCompleteAttr`): a coroutine returning it is
    /// only destroyed once suspended at its final suspend point, so its
    /// LLVM function carries `coro_only_destroy_when_complete` (WP-370).
    pub const CORO_ONLY_DESTROY_WHEN_COMPLETE: AttrKind = AttrKind(104);
    /// `optnone` / `clang::optnone` on a function (clang
    /// `OptimizeNoneAttr`): its definition is `optnone noinline` at every
    /// optimization level; it wins over `always_inline` and `minsize`
    /// (WP-405).
    pub const OPTNONE: AttrKind = AttrKind(105);
    /// `minsize` / `clang::minsize` on a function (clang `MinSizeAttr`):
    /// its definition is `minsize` (WP-405).
    pub const MINSIZE: AttrKind = AttrKind(106);
    /// GNU `naked` on a function (clang `NakedAttr`): no prologue or
    /// epilogue; the body holds only asm statements (WP-405).
    pub const NAKED: AttrKind = AttrKind(107);
    /// GNU `may_alias` on a type declaration (clang `MayAliasAttr`, no
    /// subject list): an access through the typedef or tag type is in
    /// the `omnipotent char` alias class at `-O1`+ (spec/codegen.md
    /// "TBAA"; WP-423).
    pub const MAY_ALIAS: AttrKind = AttrKind(108);
    /// GNU `nonnull` on a function or a parameter (clang `NonNullAttr`):
    /// `a` is the `ListId` of `IntegerLiteral`s holding the one-based
    /// parameter indices it names (as written until the declaration
    /// checks them); the empty list names every parameter (WP-421).
    pub const NONNULL: AttrKind = AttrKind(109);
    /// GNU `returns_nonnull` on a function (clang `ReturnsNonNullAttr`)
    /// (WP-421).
    pub const RETURNS_NONNULL: AttrKind = AttrKind(110);
    /// GNU `init_priority(N)` on a namespace-scope variable of class type
    /// (clang `InitPriorityAttr`): `a` is the priority, its dynamic
    /// initializer runs from the `_GLOBAL__I_<N>` constructor of priority
    /// `N` ([basic.start.dynamic]; spec/codegen.md §7f; WP-189).
    pub const INIT_PRIORITY: AttrKind = AttrKind(111);
    /// A loop hint (`#pragma unroll`, `nounroll`, `unroll_and_jam`,
    /// `nounroll_and_jam`, `GCC unroll`, `clang loop`) on a loop statement
    /// (clang `LoopHintAttr`, [`AttrSyntax::Pragma`]): `a` is the raw
    /// `ExprId` of the value as written (0: none), `b` the [`LoopHint`]
    /// ([`LoopHint::pack`]), `loc` the pragma-name token (`unroll`,
    /// `loop`) (WP-449).
    pub const LOOP_HINT: AttrKind = AttrKind(112);
    /// `[[clang::uninitialized]]` / `__attribute__((uninitialized))` on a
    /// local variable (clang `UninitializedAttr`): no
    /// `-ftrivial-auto-var-init` fill (spec/codegen.md §7y; WP-452).
    /// Numbered apart from the run above so a concurrent addition there
    /// does not collide.
    pub const UNINITIALIZED: AttrKind = AttrKind(140);
    /// clang `type_visibility("v")` on a class, enumeration or namespace:
    /// the visibility of the type's vtable and RTTI (`a` as for
    /// [`AttrKind::VISIBILITY`]); WP-544.
    pub const TYPE_VISIBILITY: AttrKind = AttrKind(141);
    /// clang nullability on classes: `class _Nullable X` (clang
    /// `TypeNullableAttr`, keyword syntax) lets `X _Nullable` name a
    /// nullable class type; WP-554.
    pub const TYPE_NULLABLE: AttrKind = AttrKind(142);
    /// clang `neon_vector_type(N)` (clang `NeonVectorTypeAttr`): `a` = the
    /// element-count `ExprId`. Like [`AttrKind::VECTOR_SIZE`], consumed
    /// into the declared type (a `Vector` of kind `Neon`); WP-558.
    pub const NEON_VECTOR_TYPE: AttrKind = AttrKind(143);
    /// clang `neon_polyvector_type(N)`: as [`AttrKind::NEON_VECTOR_TYPE`],
    /// a `Vector` of kind `NeonPoly`; WP-558.
    pub const NEON_POLYVECTOR_TYPE: AttrKind = AttrKind(144);
    /// GNU `target("features")` on a function (clang `TargetAttr`): `a` =
    /// the string literal's `StringLitId`. On `AArch64` codegen adds the
    /// features to the function's own (spec/landed/codegen/WP-558.md "Target
    /// features"); WP-558.
    pub const TARGET: AttrKind = AttrKind(145);
    /// GNU `gnu_inline` on an `inline` function (clang `GNUInlineAttr`):
    /// in C++ its definition is not externally visible, so codegen emits
    /// it `available_externally` or not at all (spec/codegen.md "GNU
    /// inline functions"); WP-585.
    pub const GNU_INLINE: AttrKind = AttrKind(146);
    /// GNU `nodebug` (clang `NoDebugAttr`): no debug information for the
    /// entity. kiln emits none in any case; the x86 intrinsic headers put
    /// it on every function (WP-630). Numbered apart from the run above.
    pub const NODEBUG: AttrKind = AttrKind(170);
    /// clang `min_vector_width(N)` on a function (clang
    /// `MinVectorWidthAttr`): `a` = N, the least `"min-legal-vector-width"`
    /// of its definition (WP-630).
    pub const MIN_VECTOR_WIDTH: AttrKind = AttrKind(171);
    /// GNU `weakref` / `weakref("target")` on a namespace-scope function
    /// or variable with internal linkage (clang `WeakRefAttr`). As
    /// written, `a` is the argument's `StringLitId` (0: none); the
    /// declaration's checks split `weakref("target")` into an
    /// [`AttrKind::ALIAS`] naming the target and a `WEAKREF` with `a` = 0,
    /// as clang does. A use of the declaration is a use of the symbol
    /// `target`, an `extern_weak` reference unless the symbol is also
    /// referenced or defined normally ([`crate::Ast::weakref_target`]).
    /// Numbered apart from the runs above.
    pub const WEAKREF: AttrKind = AttrKind(180);
    /// GNU `alias("target")` (clang `AliasAttr`): `a` = the target's
    /// `StringLitId`, a mangled symbol name; `b` = the raw `Loc` one past
    /// the attribute's `)` (codegen's fix-it range end). Together with `weakref` it
    /// names the weak reference's target; alone it makes the declaration
    /// a definition, emitted as an IR alias of the symbol
    /// ([`crate::Ast::alias_definition`], spec/codegen.md "Aliases and
    /// ifuncs").
    pub const ALIAS: AttrKind = AttrKind(181);
    /// GNU `analyzer_noreturn` on a function (clang
    /// `AnalyzerNoReturnAttr`): a call of it ends its block in sema's
    /// control-flow graph (as `noreturn` does), with no effect on code
    /// generation.
    pub const ANALYZER_NORETURN: AttrKind = AttrKind(182);
    /// `disable_tail_calls` / `clang::disable_tail_calls` on a function
    /// (clang `DisableTailCallsAttr`): its definition carries
    /// `"disable-tail-calls"="true"`, so the backend makes no call in it a
    /// tail call (WP-682).
    pub const DISABLE_TAIL_CALLS: AttrKind = AttrKind(183);
    /// GNU `ifunc("resolver")` on a function (clang `IFuncAttr`): `a` =
    /// the resolver's `StringLitId`, a mangled symbol name, and `b` as for
    /// [`AttrKind::ALIAS`]. The
    /// declaration is a definition, emitted as an IR ifunc whose resolver
    /// picks the implementation at load time (WP-763).
    pub const IFUNC: AttrKind = AttrKind(184);
    /// GNU `artificial` on an inline function (clang `ArtificialAttr`):
    /// debug information treats calls to it as one unit. kiln emits no
    /// debug information, so it has no effect on the IR (WP-789).
    pub const ARTIFICIAL: AttrKind = AttrKind(185);
    /// GNU `constructor` / `constructor(N)` on a function (clang
    /// `ConstructorAttr`): the definition is called before `main` from
    /// `@llvm.global_ctors` at priority `a` (65535 when none is written).
    /// As written (before `check_decl_attrs`) `a` is the raw `ExprId` of
    /// the first argument (0: none) and `b` the argument count; with
    /// [`AttrNode::PRIORITY_DEPENDENT`] `a` stays the value-dependent
    /// expression, evaluated at instantiation (WP-807).
    pub const CONSTRUCTOR: AttrKind = AttrKind(190);
    /// GNU `destructor` / `destructor(N)` (clang `DestructorAttr`): as
    /// [`AttrKind::CONSTRUCTOR`], through `@llvm.global_dtors`, after
    /// `main` returns or `exit` is called (WP-807).
    pub const DESTRUCTOR: AttrKind = AttrKind(191);
    /// GNU `cleanup(f)` on a local variable (clang `CleanupAttr`): `f(&v)`
    /// runs when the variable goes out of scope, before its destructor.
    /// As written `a` is the raw `ExprId` of the first argument (0: none)
    /// and `b` the argument count; once checked (`check_decl_attrs`) `a`
    /// is the raw `DeclId` of `f` and `b` the raw `Loc` of the argument
    /// (WP-807).
    pub const CLEANUP: AttrKind = AttrKind(192);
    /// GNU `common` (clang `CommonAttr`): rejected in C++ (clang
    /// `handleCommonAttr`), so never attached; kept as a kind for its
    /// subject check (WP-807).
    pub const COMMON: AttrKind = AttrKind(193);
    /// GNU `tls_model("model")` (clang `TLSModelAttr`) on a thread-local
    /// variable: as written `a` is the raw `ExprId` of the first argument
    /// (0: none) and `b` the argument count; once checked `a` is the
    /// model, 1-4 for `global-dynamic`, `local-dynamic`, `initial-exec`
    /// and `local-exec` (WP-807).
    pub const TLS_MODEL: AttrKind = AttrKind(194);
    /// GNU `returns_twice` (clang `ReturnsTwiceAttr`): a call of the
    /// function may return twice, like `setjmp`'s (WP-807).
    pub const RETURNS_TWICE: AttrKind = AttrKind(195);
    /// clang `pass_object_size(t)` / `pass_dynamic_object_size(t)` on a
    /// pointer parameter (clang `PassObjectSizeAttr`; WP-803): `a` = the
    /// object-size type `t` in [0, 3], `b` = 1 for the dynamic spelling.
    /// A call passes `__builtin_[dynamic_]object_size(arg, t)` of the
    /// argument as a hidden `size_t` argument after it, which a
    /// compatible `__builtin_object_size` of the parameter in the body
    /// reads; the function cannot have its address taken and overloads
    /// one without such parameters (spec/sema-overload.md §2).
    pub const PASS_OBJECT_SIZE: AttrKind = AttrKind(186);
    /// clang `overloadable` on a function (`OverloadableAttr`; WP-803):
    /// its name is mangled as C++'s even with C language linkage, and it
    /// may overload a function with C language linkage.
    pub const OVERLOADABLE: AttrKind = AttrKind(187);
    /// clang `[[clang::musttail]]` / GNU `musttail` on a return statement
    /// (clang `MustTailAttr`; WP-809): the returned call is emitted as a
    /// `musttail call` followed by `ret`. Kept on the `AttributedStmt`
    /// only once `Sema::check_musttail` accepts it.
    pub const MUSTTAIL: AttrKind = AttrKind(196);
    /// GNU `transparent_union` (clang `TransparentUnionAttr`, a C-only
    /// attribute; WP-809). In C++ every occurrence is ignored with a
    /// warning, but clang's delayed check of a class-head occurrence on a
    /// union definition still keeps it when the fields qualify: such a
    /// union is then passed by value as its first field (clang
    /// `useFirstFieldIfTransparentUnion`). Kept on the union only then.
    pub const TRANSPARENT_UNION: AttrKind = AttrKind(197);
    /// GNU / Microsoft `selectany` on a variable with external linkage
    /// (clang `SelectAnyAttr`, WP-809): its definition is `weak_odr` in a
    /// comdat of its own, so definitions in several translation units
    /// fold into one object.
    pub const SELECTANY: AttrKind = AttrKind(198);
    /// GNU `ms_struct` on a class, or implied by `#pragma ms_struct on`
    /// (clang `MSStructAttr`, WP-809): the record uses the Microsoft
    /// bit-field layout rules within the Itanium layout.
    pub const MS_STRUCT: AttrKind = AttrKind(199);
    /// GNU `gcc_struct` (clang 22 `GCCStructAttr`, WP-809): the record
    /// uses the target's default layout; conflicts with `ms_struct`.
    pub const GCC_STRUCT: AttrKind = AttrKind(215);
    /// GNU `retain` on a function or variable (clang `RetainAttr`):
    /// an emitted entity is also added to `@llvm.used`, so the linker's
    /// section garbage collection keeps it (WP-805).
    pub const RETAIN: AttrKind = AttrKind(200);
    /// clang `no_destroy` on a variable with static or thread storage
    /// duration (clang `NoDestroyAttr`): its destructor is never
    /// registered to run at exit (WP-805).
    pub const NO_DESTROY: AttrKind = AttrKind(201);
    /// clang `always_destroy` (clang `AlwaysDestroyAttr`): the variable's
    /// destructor is registered even under `-fno-c++-static-destructors`
    /// (WP-805).
    pub const ALWAYS_DESTROY: AttrKind = AttrKind(202);
    /// A calling-convention attribute written on a declaration (`ms_abi`,
    /// `preserve_most`, `regcall`, ...; clang's `handleFunctionTypeAttr`
    /// CC kinds): a function type attribute, consumed into the declared
    /// type's [`crate::types::FuncInfo`] by sema and never kept on a
    /// declaration. `a` is the [`crate::types::CallConv`] discriminant
    /// (`Default` for a convention that is the target's default, as
    /// `sysv_abi` on Linux), `b` its spelling's index in
    /// [`CC_ATTR_SPELLINGS`] (WP-808).
    pub const CALLING_CONV: AttrKind = AttrKind(210);
    /// x86 GNU `no_caller_saved_registers`: a function type attribute,
    /// consumed like [`AttrKind::CALLING_CONV`] into
    /// [`crate::types::FuncInfo::no_caller_saved_regs`] (WP-808).
    pub const NO_CALLER_SAVED_REGS: AttrKind = AttrKind(211);
    /// x86 GNU `regparm(N)`: a function type attribute, consumed like
    /// [`AttrKind::CALLING_CONV`] into [`crate::types::FuncInfo::regparm`];
    /// `a` is N (WP-808).
    pub const REGPARM: AttrKind = AttrKind(212);
    /// clang `no_builtin` / `no_builtin("name", ...)` on a function
    /// definition (clang `NoBuiltinAttr`): the named library builtins (all
    /// of them for the argument-less form, the wildcard `*`) are not
    /// recognized in its body, and its IR function and the calls of it
    /// carry `"no-builtins"` / `"no-builtin-<name>"`. `a` is the
    /// [`crate::Symbol`] index of the names, sorted, deduplicated and
    /// `,`-joined (`*` for the wildcard) (WP-808).
    pub const NO_BUILTIN: AttrKind = AttrKind(213);

    /// GNU `arm_sve_vector_bits(N)` on a typedef of a sizeless SVE type:
    /// the fixed-length vector type of N bits (WP-810; consumed into the
    /// declared type like [`AttrKind::VECTOR_SIZE`]; `a` the argument
    /// expression).
    pub const ARM_SVE_VECTOR_BITS: AttrKind = AttrKind(220);
    /// clang `__clang_arm_builtin_alias(B)` (`ArmBuiltinAliasAttr`) or
    /// `clang_builtin_alias(B)` / `[[clang::builtin_alias(B)]]`
    /// (`BuiltinAliasAttr`) on a function (WP-810): a call of the function
    /// is a call of the `AArch64` ACLE builtin `B` (`__builtin_sve_*`,
    /// `__builtin_sme_*`), with the function's declared signature; it
    /// cannot be defined or have its address taken. `a` = the `Symbol`
    /// of `B`, `b` = 1 for the `__clang_arm_builtin_alias` spelling.
    pub const BUILTIN_ALIAS: AttrKind = AttrKind(221);
    /// GNU `target_version("S")` (`TargetVersionAttr`, `AArch64` function
    /// multiversioning; WP-810). `a` = the literal's `StringLitId`, `b` =
    /// the literal's `Loc`. Kept only on a default version (`S` trimmed is
    /// `"default"`): kiln, like clang without the `fmv` target feature,
    /// hides a non-default version entirely.
    pub const TARGET_VERSION: AttrKind = AttrKind(222);
    /// GNU `target_clones("A", ...)` (`TargetClonesAttr`; WP-810). On
    /// `AArch64` without `fmv` the function is an ordinary function; `a`,
    /// `b` unused.
    pub const TARGET_CLONES: AttrKind = AttrKind(223);
    /// `cpu_dispatch(...)` / `cpu_specific(...)` (`CPUDispatchAttr`,
    /// `CPUSpecificAttr`; WP-810): never attached (invalid on `AArch64`,
    /// not supported on x86); `a` = the `Symbol` of the first CPU name,
    /// `b` = its `Loc`.
    pub const CPU_DISPATCH: AttrKind = AttrKind(224);
    /// See [`AttrKind::CPU_DISPATCH`].
    pub const CPU_SPECIFIC: AttrKind = AttrKind(225);

    /// Whether this is a calling-convention or other function type
    /// attribute sema consumes into the type's
    /// [`crate::types::FuncInfo`] ([`AttrKind::CALLING_CONV`],
    /// [`AttrKind::NO_CALLER_SAVED_REGS`], [`AttrKind::REGPARM`]).
    pub fn is_fn_type_attr(self) -> bool {
        matches!(self, AttrKind::CALLING_CONV | AttrKind::NO_CALLER_SAVED_REGS | AttrKind::REGPARM)
    }

    // WP-813: clang's class B attributes (inlining and call shape,
    // pointer facts, instrumentation and hardening), each lowered as
    // clang 19 lowers it (spec/landed/codegen/WP-813.md). Without a
    // comment, an attribute has no arguments.
    /// `no_stack_protector`: the definition has no `ssp*` attribute.
    pub const NO_STACK_PROTECTOR: AttrKind = AttrKind(240);
    /// `patchable_function_entry(N[, M])`: `a` = N, `b` = M.
    pub const PATCHABLE_FUNCTION_ENTRY: AttrKind = AttrKind(241);
    /// `zero_call_used_regs("kind")`: `a` = the kind's index in
    /// `ZERO_CALL_USED_REGS_KINDS`.
    pub const ZERO_CALL_USED_REGS: AttrKind = AttrKind(242);
    pub const SPECULATIVE_LOAD_HARDENING: AttrKind = AttrKind(243);
    pub const NO_SPECULATIVE_LOAD_HARDENING: AttrKind = AttrKind(244);
    pub const NO_SPLIT_STACK: AttrKind = AttrKind(245);
    pub const NO_INSTRUMENT_FUNCTION: AttrKind = AttrKind(246);
    pub const NO_PROFILE_INSTRUMENT_FUNCTION: AttrKind = AttrKind(247);
    /// `xray_always_instrument` (`a` = 1) / `xray_never_instrument` (0).
    pub const XRAY_INSTRUMENT: AttrKind = AttrKind(248);
    /// `xray_log_args(N)`: `a` = N.
    pub const XRAY_LOG_ARGS: AttrKind = AttrKind(249);
    pub const DISABLE_SANITIZER_INSTRUMENTATION: AttrKind = AttrKind(250);
    pub const NOCF_CHECK: AttrKind = AttrKind(251);
    /// `function_return("thunk-extern")` (`a` = 1) / `("keep")` (0).
    pub const FUNCTION_RETURN: AttrKind = AttrKind(252);
    pub const CFI_CANONICAL_JUMP_TABLE: AttrKind = AttrKind(253);
    pub const FORCE_ALIGN_ARG_POINTER: AttrKind = AttrKind(254);
    pub const FLATTEN: AttrKind = AttrKind(255);
    pub const LEAF: AttrKind = AttrKind(256);
    /// `nomerge` on a function or a statement.
    pub const NOMERGE: AttrKind = AttrKind(257);
    pub const NOT_TAIL_CALLED: AttrKind = AttrKind(258);
    pub const NODUPLICATE: AttrKind = AttrKind(259);
    pub const CONVERGENT: AttrKind = AttrKind(260);
    pub const NOUWTABLE: AttrKind = AttrKind(261);
    /// `assume_aligned(A[, O])`: `a` = A, `b` = O (both folded).
    pub const ASSUME_ALIGNED: AttrKind = AttrKind(262);
    /// `align_value(A)`: `a` = A (folded).
    pub const ALIGN_VALUE: AttrKind = AttrKind(263);
    /// `callback(callee, args...)`: `a` = the raw `ListId` of the
    /// argument indices as written (`u32`, 0 the implicit `this` or
    /// `__`'s `u32::MAX`), checked against the declaration.
    pub const CALLBACK: AttrKind = AttrKind(264);
    pub const NOESCAPE: AttrKind = AttrKind(265);
    pub const MAYBE_UNDEF: AttrKind = AttrKind(266);
    /// `[[clang::code_align(N)]]` on a loop: `a` = N.
    pub const CODE_ALIGN: AttrKind = AttrKind(267);
    pub const LOADER_UNINITIALIZED: AttrKind = AttrKind(268);
    pub const LTO_VISIBILITY_PUBLIC: AttrKind = AttrKind(269);
    /// `[[omp::assume("s")]]`: `a` = the raw `StringLitId` of `s`.
    pub const OMP_ASSUME: AttrKind = AttrKind(270);
    /// `standalone_debug` on a class (recorded for debug info).
    pub const STANDALONE_DEBUG: AttrKind = AttrKind(271);
    /// clang 22 `coro_await_elidable` on a class.
    pub const CORO_AWAIT_ELIDABLE: AttrKind = AttrKind(272);
    /// clang 22 `coro_await_elidable_argument` on a parameter.
    pub const CORO_AWAIT_ELIDABLE_ARGUMENT: AttrKind = AttrKind(273);
    /// `[[gsl::Owner(T)]]` on a class (WP-674): `b` is the raw `QualType`
    /// of `T` when [`AttrNode::GSL_HAS_TYPE`] is set.
    pub const GSL_OWNER: AttrKind = AttrKind(274);
    /// `[[gsl::Pointer(T)]]` on a class (WP-674), encoded as
    /// [`AttrKind::GSL_OWNER`].
    pub const GSL_POINTER: AttrKind = AttrKind(275);
    /// clang `unavailable` / `unavailable("message")` (WP-954): a use of
    /// the declaration is an error. `a` is the message's string-literal
    /// `ExprId`, or 0.
    pub const UNAVAILABLE: AttrKind = AttrKind(276);
    /// clang `require_constant_initialization` (WP-954), clang's
    /// `ConstInitAttr` without the `constinit` keyword: the variable,
    /// with static or thread storage duration, has constant
    /// initialization ([basic.start.static]/2).
    pub const REQUIRE_CONSTANT_INIT: AttrKind = AttrKind(277);
    /// clang `annotate("str", args...)` (WP-954): `a` is the string
    /// literal's `ExprId`, `b` the raw `ListId` of the extra arguments
    /// (constant-evaluated, or still dependent in a template), or 0.
    pub const ANNOTATE: AttrKind = AttrKind(278);

    /// Whether this is one of the vector type attributes, which are
    /// consumed into the declared type ([`AttrKind::VECTOR_SIZE`],
    /// [`AttrKind::EXT_VECTOR_TYPE`], [`AttrKind::NEON_VECTOR_TYPE`],
    /// [`AttrKind::NEON_POLYVECTOR_TYPE`]), or GNU [`AttrKind::MODE`],
    /// which is consumed the same way.
    pub fn is_vector_type(self) -> bool {
        matches!(
            self,
            AttrKind::VECTOR_SIZE
                | AttrKind::EXT_VECTOR_TYPE
                | AttrKind::NEON_VECTOR_TYPE
                | AttrKind::NEON_POLYVECTOR_TYPE
                | AttrKind::ARM_SVE_VECTOR_BITS
                | AttrKind::MODE
        )
    }

    /// Whether this is one of WP-813's class B attributes
    /// ([`AttrKind::NO_STACK_PROTECTOR`] ..=
    /// [`AttrKind::CORO_AWAIT_ELIDABLE_ARGUMENT`]), WP-675's
    /// [`AttrKind::NO_SANITIZE`], or one of WP-674's lifetime attributes
    /// spelled and checked the same way ([`AttrKind::LIFETIMEBOUND`],
    /// [`AttrKind::GSL_OWNER`], [`AttrKind::GSL_POINTER`]).
    pub fn is_class_b(self) -> bool {
        (240..=275).contains(&self.0)
            || self == AttrKind::LIFETIMEBOUND
            || self == AttrKind::NO_SANITIZE
    }
}

/// The arguments of `zero_call_used_regs` (clang `ZeroCallUsedRegsKind`),
/// in the order of [`AttrKind::ZERO_CALL_USED_REGS`]'s `a`.
pub const ZERO_CALL_USED_REGS_KINDS: [&str; 9] = [
    "skip",
    "used-gpr-arg",
    "used-gpr",
    "used-arg",
    "used",
    "all-gpr-arg",
    "all-gpr",
    "all-arg",
    "all",
];

/// The spellings of the calling-convention attributes kiln knows, by the
/// `b` word of an [`AttrKind::CALLING_CONV`] node (WP-808).
pub const CC_ATTR_SPELLINGS: [&str; 19] = [
    "ms_abi",
    "sysv_abi",
    "cdecl",
    "regcall",
    "vectorcall",
    "preserve_most",
    "preserve_all",
    "preserve_none",
    "aarch64_vector_pcs",
    "aarch64_sve_pcs",
    "intel_ocl_bicc",
    "swiftcall",
    "swiftasynccall",
    "stdcall",
    "fastcall",
    "thiscall",
    "pascal",
    "pcs",
    "m68k_rtd",
];

/// What a loop hint controls (clang `LoopHintAttr::OptionType`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kiln_snap::Snap)]
#[repr(u8)]
#[allow(missing_docs, reason = "named after clang's options")]
pub enum LoopHintOption {
    Vectorize = 0,
    VectorizeWidth,
    Interleave,
    InterleaveCount,
    Unroll,
    UnrollCount,
    UnrollAndJam,
    UnrollAndJamCount,
    PipelineDisabled,
    PipelineInitiationInterval,
    Distribute,
    VectorizePredicate,
}

impl LoopHintOption {
    const ALL: [LoopHintOption; 12] = [
        Self::Vectorize,
        Self::VectorizeWidth,
        Self::Interleave,
        Self::InterleaveCount,
        Self::Unroll,
        Self::UnrollCount,
        Self::UnrollAndJam,
        Self::UnrollAndJamCount,
        Self::PipelineDisabled,
        Self::PipelineInitiationInterval,
        Self::Distribute,
        Self::VectorizePredicate,
    ];

    /// The `#pragma clang loop` option name (clang `getOptionName`).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Vectorize => "vectorize",
            Self::VectorizeWidth => "vectorize_width",
            Self::Interleave => "interleave",
            Self::InterleaveCount => "interleave_count",
            Self::Unroll => "unroll",
            Self::UnrollCount => "unroll_count",
            Self::UnrollAndJam => "unroll_and_jam",
            Self::UnrollAndJamCount => "unroll_and_jam_count",
            Self::PipelineDisabled => "pipeline",
            Self::PipelineInitiationInterval => "pipeline_initiation_interval",
            Self::Distribute => "distribute",
            Self::VectorizePredicate => "vectorize_predicate",
        }
    }

    /// A state option (`vectorize(enable)`), else a numeric one
    /// (`vectorize_width(4)`) (clang `CheckForIncompatibleAttributes`).
    pub const fn is_state(self) -> bool {
        matches!(
            self,
            Self::Vectorize
                | Self::Interleave
                | Self::Unroll
                | Self::UnrollAndJam
                | Self::VectorizePredicate
                | Self::PipelineDisabled
                | Self::Distribute
        )
    }
}

/// A loop hint's state (clang `LoopHintAttr::LoopHintState`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kiln_snap::Snap)]
#[repr(u8)]
#[allow(missing_docs, reason = "named after clang's states")]
pub enum LoopHintState {
    Enable = 0,
    Disable,
    Numeric,
    FixedWidth,
    ScalableWidth,
    AssumeSafety,
    Full,
}

impl LoopHintState {
    const ALL: [LoopHintState; 7] = [
        Self::Enable,
        Self::Disable,
        Self::Numeric,
        Self::FixedWidth,
        Self::ScalableWidth,
        Self::AssumeSafety,
        Self::Full,
    ];
}

/// How a loop hint was spelled (clang's `LoopHintAttr` spellings).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kiln_snap::Snap)]
#[repr(u8)]
pub enum LoopHintSpelling {
    /// `#pragma clang loop`.
    ClangLoop = 0,
    /// `#pragma unroll` / `#pragma GCC unroll`.
    Unroll,
    /// `#pragma nounroll` / `#pragma GCC nounroll`.
    Nounroll,
    /// `#pragma unroll_and_jam`.
    UnrollAndJam,
    /// `#pragma nounroll_and_jam`.
    NounrollAndJam,
}

impl LoopHintSpelling {
    const ALL: [LoopHintSpelling; 5] =
        [Self::ClangLoop, Self::Unroll, Self::Nounroll, Self::UnrollAndJam, Self::NounrollAndJam];

    /// The pragma name (`"clang loop"`, `"unroll"`).
    pub const fn pragma(self) -> &'static str {
        match self {
            Self::ClangLoop => "clang loop",
            Self::Unroll => "unroll",
            Self::Nounroll => "nounroll",
            Self::UnrollAndJam => "unroll_and_jam",
            Self::NounrollAndJam => "nounroll_and_jam",
        }
    }
}

/// The `b` word of a [`AttrKind::LOOP_HINT`] attribute.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kiln_snap::Snap)]
pub struct LoopHint {
    /// The option.
    pub option: LoopHintOption,
    /// The state.
    pub state: LoopHintState,
    /// The spelling.
    pub spelling: LoopHintSpelling,
}

impl LoopHint {
    /// `option | state << 8 | spelling << 16`.
    pub const fn pack(self) -> u32 {
        self.option as u32 | (self.state as u32) << 8 | (self.spelling as u32) << 16
    }

    /// The inverse of [`LoopHint::pack`].
    pub fn unpack(b: u32) -> LoopHint {
        let at = |i: u32, n: usize| ((b >> i) & 0xff).min(n as u32 - 1) as usize;
        LoopHint {
            option: LoopHintOption::ALL[at(0, LoopHintOption::ALL.len())],
            state: LoopHintState::ALL[at(8, LoopHintState::ALL.len())],
            spelling: LoopHintSpelling::ALL[at(16, LoopHintSpelling::ALL.len())],
        }
    }
}

/// An attribute name with its `__x__` affixes removed, as clang's
/// `normalizeAttrName` does (WP-678): exactly one leading and one trailing
/// `__`, and only when both are present on a name of at least four
/// characters, so `__noinline__` is `noinline` while `__noinline`,
/// `noinline__` and `____noinline____` stay as written (and are unknown),
/// and the spellings `__const` and `__clang_arm_builtin_alias` are kept.
pub fn normalize_attr_name(name: &str) -> &str {
    if name.len() >= 4 && name.starts_with("__") && name.ends_with("__") {
        &name[2..name.len() - 2]
    } else {
        name
    }
}

/// Whether an attribute written with `syntax` and the (normalized, see
/// clang `normalizeAttrScopeName`) `scope` has its name normalized by
/// [`normalize_attr_name`]: a GNU attribute, or a `[[...]]` one with no
/// scope or the `gnu` or `clang` scope (clang `normalizeAttrName`).
pub fn attr_name_normalizes(syntax: AttrSyntax, scope: Option<&str>) -> bool {
    syntax == AttrSyntax::Gnu
        || (syntax == AttrSyntax::Cxx11 && matches!(scope, None | Some("gnu" | "clang")))
}

/// Attribute syntax.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug, kiln_snap::Snap)]
#[repr(u8)]
pub enum AttrSyntax {
    /// `[[attr]]`.
    #[default]
    Cxx11 = 0,
    /// `__attribute__((attr))`.
    Gnu = 1,
    /// A keyword (`alignas`, `_Noreturn`).
    Keyword = 2,
    /// `__declspec(attr)`.
    Declspec = 3,
    /// A pragma (`#pragma unroll`).
    Pragma = 4,
}

/// The 16-byte attribute node (spec/ast.md §7).
#[derive(Clone, Copy, PartialEq, Eq, Debug, kiln_snap::Snap)]
pub struct AttrNode {
    /// The kind.
    pub kind: AttrKind,
    /// The syntax.
    pub syntax: AttrSyntax,
    /// Per-kind flags (bit 0: implicit; bit 1: inherited from a previous
    /// declaration).
    pub flags: u8,
    /// The attribute-token location.
    pub loc: Loc,
    /// Argument word `a` (an `ExprId`, `Symbol`, `ConstValueId` or side
    /// index, per kind).
    pub a: u32,
    /// Argument word `b`.
    pub b: u32,
}

impl AttrNode {
    /// `[[assume]]` flag (bit 2): the assumption has possible side
    /// effects and is ignored (clang `-Wassume`): neither assumed by
    /// codegen nor checked by constant evaluation. For `ASSUME`, `a` is
    /// the assumption (`ExprId`) and `b` the raw `Loc` of its clang
    /// `getExprLoc`, where constant evaluation reports it false.
    pub const ASSUME_IGNORED: u8 = 4;
    /// `[[fallthrough]]` flag (bit 2): written with an attribute scope
    /// (`[[clang::fallthrough]]`, `[[gnu::fallthrough]]`), so not a C++17
    /// extension before C++17 (clang `handleFallThroughAttr`).
    pub const FALLTHROUGH_SCOPED: u8 = 4;
    /// `constructor(N)` / `destructor(N)` flag (bit 2): `N` is
    /// value-dependent (clang 22 accepts it; clang 19 rejects it), so `a`
    /// is the expression, checked when the function is instantiated.
    pub const PRIORITY_DEPENDENT: u8 = 4;
    /// GNU `mode` flag (bit 2): it modifies a dependent type, so it stays
    /// on the declaration and applies at instantiation (clang
    /// `instantiateDependentModeAttr`).
    pub const MODE_DEFERRED: u8 = 4;
    /// `gsl::Owner` / `gsl::Pointer` flag (bit 2): a type argument was
    /// written; `b` holds it.
    pub const GSL_HAS_TYPE: u8 = 4;
    /// `lifetimebound` flag (bit 2): an argument list was written, an
    /// error on a parameter but unchecked in the type position (clang
    /// does not count the arguments of a type attribute).
    pub const LIFETIMEBOUND_ARGS: u8 = 4;
    /// Flag (bit 3, any kind): written with the `gnu::` attribute scope
    /// (`[[gnu::noreturn]]`). With `AttrSyntax::Gnu` it selects the GNU
    /// spelling of `NODISCARD` (`warn_unused_result`) and `NORETURN`, and
    /// after a function declarator it makes a misplaced attribute a
    /// warning rather than an error (clang `isGNUScope`).
    pub const GNU_SCOPED: u8 = 8;

    /// Implicit flag (bit 0): added by sema, not written.
    pub const IMPLICIT: u8 = 1;

    /// The visibility of a `VISIBILITY` / `TYPE_VISIBILITY` node: 0
    /// default, 1 hidden, 2 protected.
    pub const fn visibility(&self) -> u32 {
        self.a
    }

    /// True for the GNU spelling: `__attribute__((x))` or `[[gnu::x]]`.
    pub const fn is_gnu_spelling(&self) -> bool {
        matches!(self.syntax, AttrSyntax::Gnu) || self.flags & Self::GNU_SCOPED != 0
    }
}

/// Attribute nodes and the declaration → attributes map.
#[derive(Debug, kiln_snap::Snap)]
pub struct AttrArena {
    nodes: Vec<AttrNode>,
    /// The kinds among `nodes`, one bit per [`AttrKind`] (a node's kind
    /// never changes once pushed), for [`AttrArena::has_kind`].
    kinds: Vec<u64>,
    /// Attributes of declarations that have any (`HAS_ATTRS`).
    pub decl_attrs: ColdMap<DeclId, ListId>,
}

impl Default for AttrArena {
    fn default() -> Self {
        Self {
            nodes: vec![AttrNode {
                kind: AttrKind(0),
                syntax: AttrSyntax::Cxx11,
                flags: 0,
                loc: Loc::INVALID,
                a: 0,
                b: 0,
            }],
            kinds: Vec::new(),
            decl_attrs: ColdMap::new("decl_attrs"),
        }
    }
}

impl AttrArena {
    /// An arena holding only the dummy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends an attribute.
    pub fn push(&mut self, a: AttrNode) -> AttrId {
        let id = AttrId::new(self.nodes.len() as u32);
        let (word, bit) = (usize::from(a.kind.0 / 64), a.kind.0 % 64);
        if self.kinds.len() <= word {
            self.kinds.resize(word + 1, 0);
        }
        self.kinds[word] |= 1 << bit;
        self.nodes.push(a);
        id
    }

    /// The attribute `a`.
    pub fn get(&self, a: AttrId) -> &AttrNode {
        &self.nodes[a.index() as usize]
    }

    /// Sets the flag bits `flags` on attribute `a` (while it is being
    /// built, before it is attached).
    pub fn set_flags(&mut self, a: AttrId, flags: u8) {
        self.nodes[a.index() as usize].flags |= flags;
    }

    /// Replaces the argument words of attribute `a` (while it is being
    /// built, before it is attached).
    pub fn set_args(&mut self, a: AttrId, x: u32, y: u32) {
        let n = &mut self.nodes[a.index() as usize];
        n.a = x;
        n.b = y;
    }

    /// The number of attributes.
    pub fn len(&self) -> usize {
        self.nodes.len() - 1
    }

    /// True if there are no attributes.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether any attribute of kind `kind` exists.
    pub fn has_kind(&self, kind: AttrKind) -> bool {
        self.kinds.get(usize::from(kind.0 / 64)).is_some_and(|w| w & (1 << (kind.0 % 64)) != 0)
    }

    /// The attribute list of `d` ([`ListId::EMPTY`] if none).
    pub fn of_decl(&self, d: DeclId) -> ListId {
        self.decl_attrs.get(d).copied().unwrap_or(ListId::EMPTY)
    }

    pub(crate) fn live_bytes(&self) -> u64 {
        crate::mem::VecDims::of("nodes", &self.nodes).live_bytes()
            + crate::mem::VecDims::of("kinds", &self.kinds).live_bytes()
            + self.decl_attrs.live_bytes()
    }
}

impl HeapSize for AttrArena {
    fn heap_bytes(&self) -> MemReport {
        let mut r = MemReport::new();
        r.push(MemRow::of_vec("nodes", &self.nodes));
        r.push(MemRow::of_vec("kinds", &self.kinds));
        r.push(self.decl_attrs.mem_row());
        r
    }
}
