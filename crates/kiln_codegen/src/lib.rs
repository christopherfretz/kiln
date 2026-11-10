//! Lowering of the typed AST to LLVM IR.
//!
//! The lowering mirrors clang 19 at `-O0` instruction for instruction so
//! that kiln's output can be diffed against the clang oracle; see
//! `spec/codegen.md` for the contract this crate implements.
#![forbid(unsafe_code)]
// The emitters mirror clang's CodeGen function by function: long
// per-construct dispatch functions, case arms kept 1:1 with clang's even
// when two bodies coincide, and the short operand names (`l`, `r`, `q`,
// `t`, `v`) of IR builder code. Emitters return `R<_>` uniformly even
// where the M2 subset cannot fail yet.
#![allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::match_same_arms,
    clippy::unnecessary_wraps
)]

mod alias;
mod annotate;
mod arm;
mod asm;
mod atomic;
mod auto_init;
mod builtins;
mod checked;
mod class;
mod class_b;
mod cleanup;
mod cleanup_attr;
mod clear_padding;
mod complex;
mod consts;
mod contracts;
mod coro;
mod coverage;
mod debuginfo;
mod dyncast;
mod eh;
mod expr;
mod fir;
mod fold;
mod fp;
mod func;
mod global_init;
pub mod ignorelist;
mod lambda;
mod lifetime;
mod loop_md;
mod memptr;
mod module;
mod module_init;
mod neon;
mod new_delete;
mod null_init;
mod outline;
mod psabi;
mod recursion;
mod stmt;
mod sve;
mod target_features;
mod tbaa;
mod tls;
mod try_catch;
mod ubsan;
mod va_arg;
mod visibility;
mod vla;
mod vtable;
mod x86;
mod x86_gen;
mod x86_intrinsics;
mod x86_mmx;

/// The x86 builtins clang lowers to one `llvm.x86.*` call
/// (spec/landed/codegen/WP-630.md "1:1 intrinsics"), decoded from their
/// plain-data table (spec/landed/codegen/WP-660.md); public for the table
/// tests.
pub mod x86_intrinsic_table {
    pub use crate::x86::{Map, by_name, len, rows};
    pub use kiln_llvm::intrinsics::TargetMem as Mem;
}

pub use builtins::{UNLOWERED_BUILTINS, is_unlowered_builtin};
pub use coverage::CoverageOptions;
pub use debuginfo::{DebugInfoKind, DebugOptions};
use kiln_abi::Target;
use kiln_ast::Ast;
use kiln_llvm::Module;
pub use kiln_llvm::Visibility;
use kiln_oven::Oven;
use kiln_source::Loc;

/// Options controlling code generation.
#[derive(Clone, Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent command-line switches, as clang's CodeGenOptions"
)]
pub struct CodegenOptions {
    /// The module's `source_filename`.
    pub source_filename: String,
    /// C++ exceptions are enabled (`-fexceptions`, the default). When
    /// disabled every function is `nounwind`.
    pub exceptions: bool,
    /// RTTI is enabled (`-frtti`, the default). When disabled (`-fno-rtti`)
    /// a vtable's RTTI slot is null and `type_info` objects are emitted
    /// only for exception handling, as `linkonce_odr` definitions in every
    /// TU that needs one, never as references to a key-function TU (clang
    /// `shouldEmitRTTI`, `ShouldUseExternalRTTIDescriptor`,
    /// `getTypeInfoLinkage`; spec/codegen.md "RTTI").
    pub rtti: bool,
    /// The `llvm.ident` string; empty for none.
    pub ident: String,
    /// `AArch64` only: `+outline-atomics` (`Some(true)`: `-moutline-atomics`,
    /// or the default with a GCC installation of 9.3.1 or later) or
    /// `-outline-atomics` (`Some(false)`: `-mno-outline-atomics`) in the
    /// functions' `target-features`; `None` names neither (clang
    /// `getAArch64TargetFeatures`). Ignored on x86-64.
    pub outline_atomics: Option<bool>,
    /// On `AArch64`, the `+name`/`-name` target features beyond the base
    /// set (`-march=armv8-a+sve`, `-target-feature`; one per name), merged
    /// into the functions' `target-features`. On x86-64, the module's
    /// whole feature map as `+name`/`-name` (empty: the default
    /// `x86-64`'s; WP-630, WP-1000).
    pub target_features: Vec<String>,
    /// x86-64: the module CPU's features (`-march=`; empty: the default
    /// `x86-64`, or `native` off an x86-64 host), the start of a
    /// `target("...")` function's map without `arch=` (WP-1000).
    pub target_cpu_features: Vec<String>,
    /// x86-64: the command line's feature items as written (`-m<feature>`
    /// as `+name`/`-name`; clang `FeaturesAsWritten`), applied before a
    /// `target("...")` attribute's own items (WP-1000).
    pub target_features_written: Vec<String>,
    /// How a function's `target("...")` attribute string changes the
    /// module's features (the driver's `features::target_attr_features` /
    /// `features::x86_target_attr_features`; see [`TargetAttrModule`]).
    /// `None` ignores the attribute (spec/landed/codegen/WP-558.md "Target
    /// features", WP-558; spec/landed/codegen/WP-630.md, WP-630).
    pub target_attr: Option<TargetAttrParser>,
    /// `AArch64` only: the `sve` target feature is on, so function
    /// definitions carry `vscale_range(1,16)` (clang `getVScaleRange`).
    pub sve: bool,
    /// `AArch64` only: `-msve-vector-bits=N`, so function definitions
    /// carry `vscale_range(N/128,N/128)` with or without `sve`.
    pub sve_vector_bits: Option<u32>,
    /// How `_Complex` floating multiply and divide are lowered
    /// (`-fcx-limited-range`, `-fcomplex-arithmetic=`; clang's
    /// `LangOptions::ComplexRange`).
    pub complex_range: ComplexRange,
    /// The command line's floating-point semantics (`-ffp-contract=`,
    /// `-ffast-math` and its parts, `-f[no-]math-errno`; spec/codegen.md
    /// "Floating-point semantics", WP-806).
    pub fp: kiln_ast::fp::FpOptions,
    /// The `-fdenormal-fp-math=` mode as `"output,input"`
    /// (`"denormal-fp-math"` on every function); `None` is IEEE.
    pub fp_denormal: Option<String>,
    /// The optimization level (clang `CodeGenOpts.OptimizationLevel`):
    /// 0 at `-O0`, 1 at `-O`/`-O1`/`-Og`, 2 at `-O2`/`-Os`/`-Oz`, 3 at
    /// `-O3` and above. Above 0 the IR will be optimized (see
    /// [`Self::optimize`]); at exactly 1 loops carry
    /// `llvm.loop.unroll.disable` (spec/codegen.md "Loop metadata").
    pub opt_level: u8,
    /// The size optimization level: 1 at `-Os`, 2 at `-Oz`, else 0
    /// (clang `CodeGenOpts.OptimizeSize`): functions and call sites get
    /// `optsize`, and `minsize` too at 2 (spec/codegen.md "Size
    /// optimization attributes").
    pub opt_size: u8,
    /// Type-based alias analysis applies (`-fstrict-aliasing`, the
    /// default; `-fno-strict-aliasing` is clang's `RelaxedAliasing`):
    /// with [`Self::optimize`], loads and stores carry `!tbaa`
    /// (spec/codegen.md "TBAA").
    pub strict_aliasing: bool,
    /// The clang version whose diagnostic text codegen's warnings follow
    /// (`-fclang-compat=`; spec/driver.md).
    pub clang_compat: u32,
    /// The major version of the LLVM that consumes the IR (`llvm-config
    /// --version` of the driver's `KILN_LLVM_DIR`; spec/driver.md): selects
    /// the IR that changed between the supported versions (19 and 22).
    pub llvm_major: u32,
    /// The last of `-fomit-frame-pointer` (`Some(true)`) and
    /// `-fno-omit-frame-pointer` (`Some(false)`); `None` is the target's
    /// default ([`FramePointer::resolve`]).
    pub omit_frame_pointer: Option<bool>,
    /// The last of `-momit-leaf-frame-pointer` (`Some(true)`) and
    /// `-mno-omit-leaf-frame-pointer` (`Some(false)`); `None` is the
    /// target's default.
    pub omit_leaf_frame_pointer: Option<bool>,
    /// What an automatic variable without an initializer holds at its
    /// declaration (`-ftrivial-auto-var-init=`; the C++26 erroneous value
    /// of [basic.indet], spec/codegen.md §7r). The driver's default is
    /// [`AutoVarInit::Zero`] in C++26 and [`AutoVarInit::Uninitialized`]
    /// before.
    pub auto_var_init: AutoVarInit,
    /// `-ftrivial-auto-var-init-max-size=`: a variable larger than this
    /// many bytes is not initialized (0: no limit).
    pub auto_var_init_max_size: u64,
    /// The visibility of a value (function, variable) that names none
    /// (`-fvisibility=`; clang `LangOptions::ValueVisibilityMode`;
    /// spec/codegen.md "Symbol visibility").
    pub visibility: Visibility,
    /// The visibility of a type's symbols (vtable, VTT, RTTI) that name
    /// none: [`Self::visibility`], except default under
    /// `-fvisibility-ms-compat` (clang `TypeVisibilityMode`).
    pub type_visibility: Visibility,
    /// `-fvisibility-inlines-hidden`: an inline member function that
    /// names no visibility is hidden (clang `InlineVisibilityHidden`).
    pub inlines_hidden: bool,
    /// The relocation model (`-fpic`/`-fPIC`/`-fpie`/`-fPIE`/`-fno-pic`):
    /// the `PIC Level`/`PIE Level` module flags and which globals are
    /// `dso_local` (clang `shouldAssumeDSOLocal`).
    pub reloc: RelocModel,
    /// `-fsemantic-interposition` (`Some(true)`), `-fno-semantic-interposition`
    /// (`Some(false)`: an external default-visibility function definition
    /// is `dso_local` under `-fpic`) or neither (clang's
    /// `-fhalf-no-semantic-interposition`); only `-fpic` reads it.
    pub semantic_interposition: Option<bool>,
    /// `-f[no-]direct-access-external-data`; `None` is clang's default (on
    /// only for `-fno-pic`): a non-TLS variable declaration is `dso_local`.
    pub direct_access_external_data: Option<bool>,
    /// `-fstack-protector[-strong|-all]` / `-fno-stack-protector`: the
    /// `ssp` / `sspstrong` / `sspreq` attribute of every function
    /// definition (clang `SetLLVMFunctionAttributesForDefinition`;
    /// spec/codegen.md "Hardening and overflow options").
    pub stack_protector: StackProtector,
    /// The `uwtable` attribute of definitions and the `uwtable` module
    /// flag (`-f[no-]asynchronous-unwind-tables`, `-f[no-]unwind-tables`;
    /// clang `CodeGenOpts.UnwindTables`): `None` emits neither.
    pub unwind_tables: Option<UnwindTables>,
    /// Which static and thread-local variables register their
    /// destructors (`-f[no-]c++-static-destructors[=…]`; WP-805).
    pub static_destructors: StaticDestructors,
    /// `-fno-plt`: a function declaration with no definition in the
    /// translation unit is `nonlazybind`, and the module carries
    /// `RtLibUseGOT` (clang `CodeGenOpts.NoPLT`).
    pub no_plt: bool,
    /// Signed integer overflow is defined (`-fwrapv`,
    /// `-fno-strict-overflow`; clang `SOB_Defined`): no `nsw` on signed
    /// arithmetic.
    pub wrapv: bool,
    /// Pointer overflow is defined (clang 19's `-fwrapv`, clang 20+'s
    /// `-fwrapv-pointer`, and `-fno-strict-overflow`; clang
    /// `PointerOverflowDefined`): pointer arithmetic and subscripts are
    /// plain `getelementptr` (no `inbounds`), and a variable length
    /// array index is scaled by a plain `mul`.
    pub wrapv_pointer: bool,
    /// `-ftrapv` (clang `SOB_Trapping`; WP-1032): signed `+ - *`,
    /// negation, `++` and `--` check overflow and trap (`llvm.ubsantrap`)
    /// unless `signed-integer-overflow` reports it. Overrides
    /// [`Self::wrapv`].
    pub trapv: bool,
    /// `-ftrapv-handler=<name>` with `-ftrapv` (clang
    /// `OverflowHandler`): an overflow calls `i64 <name>(i64, i64, i8,
    /// i8, ...)` for the result instead of trapping.
    pub trapv_handler: Option<String>,
    /// `-fno-delete-null-pointer-checks` (clang
    /// `CodeGenOpts.NullPointerIsValid`): functions are
    /// `null_pointer_is_valid`, and references, `this` and `operator new`
    /// carry no `nonnull` (`this` is `dereferenceable_or_null`).
    pub null_pointer_is_valid: bool,
    /// `-fno-builtin` / `-ffreestanding` (clang `LangOpts.NoBuiltin`):
    /// every function and call site is `"no-builtins"`, a call site also
    /// `nobuiltin`, and no library function is lowered as its builtin
    /// (spec/codegen.md "no-builtin"; WP-808).
    pub no_builtin: bool,
    /// The `-fno-builtin-<name>` names, sorted (clang
    /// `LangOpts.NoBuiltinFuncs`): `"no-builtin-<name>"` on every function
    /// and call site, `nobuiltin` on a call of `<name>` itself.
    pub no_builtin_fns: Vec<String>,
    /// The x86-64 `-march=<cpu>`: the functions' `target-cpu` (and no
    /// default `tune-cpu`, as clang); `None` is the target's default
    /// (`x86-64`, `generic` on `AArch64`). `native` (both targets) emits
    /// no `target-cpu`, so the backend's `-mcpu=native` decides.
    pub target_cpu: Option<String>,
    /// `-mtune=<cpu>`: the functions' `tune-cpu`; `None` is the target's
    /// default (`generic` on x86-64 without `-march=`, none on `AArch64`).
    pub tune_cpu: Option<String>,
    /// `-fno-inline` (clang `CodeGenOptions::OnlyAlwaysInlining`): every
    /// definition but an `always_inline` one is `noinline`, at any `-O`.
    pub no_inline: bool,
    /// `-mcmodel=`: the `Code Model` module flag (and, on x86-64, `Large
    /// Data Threshold` for `medium` / `large`, as clang); `None` emits
    /// neither (the backend's default, `small`).
    pub code_model: Option<CodeModel>,
    /// How `std::meta::info` template arguments mangle
    /// (`-fkiln-reflection-mangling=`; spec/reflection.md §8.3).
    pub reflection_mangling: kiln_abi::ReflectionMangling,
    /// `-fkiln-outline-expansions=<N>`: an expanded expansion statement
    /// whose elements total at least `N` statements and expressions has
    /// each element emitted into an internal helper (above `-O0`; `0`
    /// never outlines; spec/landed/codegen/WP-793.md).
    pub outline_expansions: usize,
    /// Define the default contract-violation handler (weak
    /// `handle_contract_violation` and
    /// `std::contracts::invoke_default_contract_violation_handler`) in a
    /// translation unit that checks a contract assertion: every stdlib
    /// but libstdc++, which ships its own (spec/contracts.md §6.3).
    pub contract_default_handler: bool,
    /// The language has C++20 modules: C++20 or later (clang
    /// `LangOptions::CPlusPlusModules`, which turns on `Modules`). Before
    /// clang 20 that is what emits the empty `!llvm.linker.options`
    /// (spec/codegen.md §2).
    pub cxx_modules: bool,
    /// Debug information (`-g` and friends; spec/codegen.md "Debug
    /// information", WP-814).
    pub debug: DebugOptions,
    /// Coverage instrumentation (WP-941, spec/codegen.md
    /// "Instrumentation").
    pub coverage: CoverageOptions,
    /// The hardening and instrumentation flags (WP-813).
    pub hardening: Hardening,
    /// The `-fsanitize=` set (WP-675, spec/codegen.md "Sanitizers").
    pub sanitize: SanitizeOptions,
    /// The `-fsanitize-ignorelist=` and default lists (WP-1032): what
    /// they name is not instrumented.
    pub ignorelist: Option<std::sync::Arc<ignorelist::SanitizerIgnorelist>>,
    /// `-flto` / `-flto=thin`: the module is an LTO unit (its module flags
    /// and vtable `!type` metadata; spec/codegen.md §5.3 "LTO", WP-873).
    pub lto: Option<Lto>,
    /// With [`Self::lto`], the module flags clang's backend pipeline adds
    /// (`ThinLTO`, `EnableSplitLTOUnit`); off under `-disable-llvm-passes`.
    pub lto_module_flags: bool,
}

/// The kind of LTO a module is compiled for (clang
/// `CodeGenOpts.PrepareForLTO` / `PrepareForThinLTO`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lto {
    /// `-flto` / `-flto=full`: `ThinLTO` 0 and `EnableSplitLTOUnit` 1.
    Full,
    /// `-flto=thin`; the field is `-fsplit-lto-unit` (`EnableSplitLTOUnit`).
    Thin {
        /// `-fsplit-lto-unit` was the last of it and `-fno-split-lto-unit`.
        split: bool,
    },
}

/// The resolved sanitizer options (WP-675, docs/design/sanitizers.md).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SanitizeOptions {
    /// The enabled sanitizers.
    pub kinds: kiln_ast::sanitize::SanitizerSet,
    /// The UB checks that report and continue (`-fsanitize-recover=`).
    pub recover: kiln_ast::sanitize::SanitizerSet,
    /// The UB checks that trap (`-fsanitize-trap=`).
    pub trap: kiln_ast::sanitize::SanitizerSet,
    /// The checks whose traps and handler calls may merge
    /// (`-f[no-]sanitize-merge[=]`, WP-1032); `None` when no such flag is
    /// given (clang 22: the `undefined` group; clang 19: every check).
    pub merge: Option<kiln_ast::sanitize::SanitizerSet>,
    /// clang `SanitizeAddressUseAfterScope`: an address sanitizer with
    /// `-fsanitize-address-use-after-scope` (the default) wants lifetime
    /// markers.
    pub use_after_scope: bool,
    /// The unit is C++ (`UBSan` `shift-base` tolerates a shift into the
    /// sign bit there).
    pub cplusplus: bool,
    /// The unit is C++20 or later (signed `<<` is defined: no
    /// `shift-base` check of signed shifts).
    pub cxx20: bool,
}

impl SanitizeOptions {
    /// Whether any sanitizer is on.
    #[must_use]
    pub fn any(&self) -> bool {
        !self.kinds.is_empty()
    }
    /// Whether `k` is on.
    #[must_use]
    pub fn has(&self, k: kiln_ast::sanitize::SanitizerSet) -> bool {
        self.kinds.has_any(k)
    }
}

/// The hardening and instrumentation driver flags behind the class B
/// attributes (WP-813, spec/landed/codegen/WP-813.md).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools, reason = "independent command-line switches")]
pub struct Hardening {
    /// `-fsplit-stack`.
    pub split_stack: bool,
    /// `-mspeculative-load-hardening`.
    pub speculative_load_hardening: bool,
    /// `-fzero-call-used-regs=<kind>`.
    pub zero_call_used_regs: Option<&'static str>,
    /// `-mfunction-return=thunk-extern` (x86).
    pub fn_return_thunk_extern: bool,
    /// `-fpatchable-function-entry=<count>[,<offset>]`.
    pub patchable_function_entry: Option<(u32, u32)>,
    /// `-fxray-instrument`, with its options.
    pub xray: Option<Xray>,
    /// `-finstrument-functions`.
    pub instrument_functions: bool,
    /// `-finstrument-functions-after-inlining`.
    pub instrument_functions_after_inlining: bool,
    /// `-finstrument-function-entry-bare`.
    pub instrument_function_entry_bare: bool,
    /// `-fcf-protection=branch` or `full` (x86).
    pub cf_protection_branch: bool,
    /// `-fcf-protection=return` or `full` (x86).
    pub cf_protection_return: bool,
    /// `-mbranch-protection=` / `-msign-return-address=` (`AArch64`;
    /// WP-846).
    pub branch_protection: kiln_abi::BranchProtection,
}

/// The `-fxray-instrument` options.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Xray {
    /// `-fxray-instruction-threshold=` (default 200).
    pub threshold: u32,
    /// `-fxray-ignore-loops`.
    pub ignore_loops: bool,
}

impl Default for Xray {
    fn default() -> Self {
        Xray { threshold: 200, ignore_loops: false }
    }
}

/// An `-mcmodel=` code model (LLVM `CodeModel::Model`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeModel {
    /// `tiny` (`AArch64`).
    Tiny,
    /// `small`, the default.
    Small,
    /// `kernel` (x86-64).
    Kernel,
    /// `medium`.
    Medium,
    /// `large`.
    Large,
}

impl CodeModel {
    /// The `-mcmodel=` spelling, which is also `llc -code-model=`'s.
    pub fn from_name(name: &str) -> Option<CodeModel> {
        Some(match name {
            "tiny" => CodeModel::Tiny,
            "small" => CodeModel::Small,
            "kernel" => CodeModel::Kernel,
            "medium" => CodeModel::Medium,
            "large" => CodeModel::Large,
            _ => return None,
        })
    }

    /// The `llc -code-model=` value.
    pub fn name(self) -> &'static str {
        match self {
            CodeModel::Tiny => "tiny",
            CodeModel::Small => "small",
            CodeModel::Kernel => "kernel",
            CodeModel::Medium => "medium",
            CodeModel::Large => "large",
        }
    }

    /// The `Code Model` module flag's value (LLVM's enumerator).
    pub fn flag_value(self) -> i32 {
        match self {
            CodeModel::Tiny => 0,
            CodeModel::Small => 1,
            CodeModel::Kernel => 2,
            CodeModel::Medium => 3,
            CodeModel::Large => 4,
        }
    }
}

/// The stack protector level (clang `LangOptions::StackProtectorMode`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StackProtector {
    /// `-fno-stack-protector`, the default: no attribute.
    #[default]
    Off,
    /// `-fstack-protector`: `ssp`.
    On,
    /// `-fstack-protector-strong`: `sspstrong`.
    Strong,
    /// `-fstack-protector-all`: `sspreq`.
    All,
}

/// The kind of unwind table definitions carry (clang
/// `CodeGenOptions::UnwindTables`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UnwindTables {
    /// `-funwind-tables` without asynchronous tables: `uwtable(sync)` and
    /// module flag `uwtable` 1.
    Sync,
    /// `-fasynchronous-unwind-tables`, the hosted default: `uwtable` and
    /// module flag `uwtable` 2.
    #[default]
    Async,
}

/// The relocation model and its level (clang `-pic-level`, `-pic-is-pie`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelocModel {
    /// `-fno-pic`: a position-dependent executable.
    Static,
    /// `-fpic` (1) / `-fPIC` (2): a shared object.
    Pic(u8),
    /// `-fpie` (1) / `-fPIE` (2, the default): a position-independent
    /// executable.
    Pie(u8),
}

impl Default for RelocModel {
    fn default() -> Self {
        RelocModel::Pie(2)
    }
}

/// clang's `LangOptions::TrivialAutoVarInitKind`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AutoVarInit {
    /// Left uninitialized (indeterminate).
    #[default]
    Uninitialized,
    /// All-zero bytes, padding included.
    Zero,
    /// clang's repeated pattern (`initializationPatternFor`): `0xAA`
    /// bytes for integers, pointers and padding, all-ones NaNs for
    /// floating point (spec/codegen.md §7y).
    Pattern,
}

/// kiln's default `-ftrivial-auto-var-init-max-size` (spec/codegen.md
/// §7r): 4 KiB.
pub const AUTO_VAR_INIT_MAX_SIZE: u64 = 4096;

/// The default `-fkiln-outline-expansions=` threshold (measured,
/// spec/landed/codegen/WP-793.md).
pub const OUTLINE_EXPANSIONS_DEFAULT: usize = 2048;

/// Which functions keep a frame pointer: the `"frame-pointer"` function
/// attribute and the `frame-pointer` module flag (clang's
/// `CodeGenOptions::FramePointerKind`; spec/codegen.md "Frame pointer").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FramePointer {
    /// No function keeps one: neither the attribute nor the flag is
    /// emitted.
    None,
    /// Every function but a leaf keeps one (`"non-leaf"`, flag 1).
    NonLeaf,
    /// clang 22's spelling of `NonLeaf`: the frame-pointer register is
    /// not reserved in leaf functions (`"non-leaf-no-reserve"`, flag 4).
    NonLeafNoReserve,
    /// Every function keeps one (`"all"`, flag 2).
    All,
}

impl FramePointer {
    /// clang's driver rule (`getFramePointerKind`): the frame pointer is
    /// omitted when `-fomit-frame-pointer` is last, kept when
    /// `-fno-omit-frame-pointer` is, and otherwise by the target's
    /// default: x86-64 Linux omits it when optimizing (any `-O` above
    /// `-O0`), `AArch64` Linux always keeps it. A kept frame pointer is
    /// omitted in leaf functions when `-momit-leaf-frame-pointer` is
    /// last, kept with `-mno-omit-leaf-frame-pointer`, and by default
    /// omitted on `AArch64` and kept on x86-64.
    #[must_use]
    pub fn resolve(
        x86_64: bool,
        optimize: bool,
        omit: Option<bool>,
        omit_leaf: Option<bool>,
    ) -> FramePointer {
        if omit.unwrap_or(x86_64 && optimize) {
            FramePointer::None
        } else if omit_leaf.unwrap_or(!x86_64) {
            FramePointer::NonLeaf
        } else {
            FramePointer::All
        }
    }

    /// The `"frame-pointer"` attribute value, `None` when none is
    /// emitted.
    #[must_use]
    pub fn attr(self) -> Option<&'static str> {
        match self {
            FramePointer::None => None,
            FramePointer::NonLeaf => Some("non-leaf"),
            FramePointer::NonLeafNoReserve => Some("non-leaf-no-reserve"),
            FramePointer::All => Some("all"),
        }
    }

    /// The `frame-pointer` module flag value, `None` when none is
    /// emitted.
    #[must_use]
    pub fn module_flag(self) -> Option<i32> {
        match self {
            FramePointer::None => None,
            FramePointer::NonLeaf => Some(1),
            FramePointer::NonLeafNoReserve => Some(4),
            FramePointer::All => Some(2),
        }
    }
}

/// Which variables with static or thread storage duration register
/// their destructors (`-fc++-static-destructors=`; clang
/// `LangOptions::RegisterStaticDestructorsKind`, `VarDecl::isNoDestroy`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StaticDestructors {
    /// All of them (the default).
    #[default]
    All,
    /// Only `thread_local` ones (`=thread-local`, clang 22).
    ThreadLocal,
    /// None of them (`-fno-c++-static-destructors`, `=none`), except
    /// `[[clang::always_destroy]]` ones.
    None,
}

/// clang's `LangOptions::ComplexRangeKind` (spec/abi.md §8.17): the
/// algorithm for `_Complex` floating `*` and `/` when both operands are
/// complex.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ComplexRange {
    /// `full` (the default, `-fno-cx-limited-range`): the algebraic
    /// product with a NaN check falling back to `__mul?c3`; division by
    /// `__div?c3`.
    #[default]
    Full,
    /// `improved` (`-fcx-fortran-rules`): the algebraic product; Smith's
    /// range-reducing division (`EmitRangeReductionDiv`).
    Improved,
    /// `basic` (`-fcx-limited-range`, `-ffast-math`): the algebraic
    /// product and quotient (`EmitAlgebraicDiv`).
    Basic,
}

impl ComplexRange {
    /// The `-fcomplex-arithmetic=` value kiln supports (`promoted` is
    /// not: it widens the operands, spec/abi.md §8.17).
    #[must_use]
    pub fn from_arg(v: &str) -> Option<ComplexRange> {
        match v {
            "full" => Some(ComplexRange::Full),
            "improved" => Some(ComplexRange::Improved),
            "basic" => Some(ComplexRange::Basic),
            _ => None,
        }
    }
}

/// The target features of one `target("...")` attribute string (clang
/// `TargetInfo::parseTargetAttr`, then `initFeatureMap`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TargetAttrFeatures {
    /// The `+name` / `-name` entries applied in order over the module's
    /// features (the last naming a feature wins). On `AArch64`: the
    /// attribute's non-extension features (an architecture level with the
    /// features it implies), then every extension feature of the module or
    /// the attribute with its final state. On x86-64: the attribute's own
    /// features, then the function's whole map. The `+` ones are what an
    /// `always_inline` callee requires first, in this order.
    pub features: Vec<String>,
    /// The function's `target-cpu`, which drops the default `tune-cpu`:
    /// on x86-64 the CPU of an `arch=<cpu>` item (WP-630), on `AArch64` a
    /// known CPU of a `cpu=<cpu>` item, as written (WP-1187).
    pub cpu: Option<String>,
    /// The known CPU of a `tune=<cpu>` item, the function's `tune-cpu`.
    pub tune: Option<String>,
}

/// The module side of a `target("...")` attribute (see
/// [`CodegenOptions::target_attr`]).
#[derive(Clone, Copy, Debug)]
pub struct TargetAttrModule<'a> {
    /// The module's features as `+name`/`-name`.
    pub features: &'a [String],
    /// x86-64: [`CodegenOptions::target_cpu_features`].
    pub cpu_features: &'a [String],
    /// x86-64: [`CodegenOptions::target_features_written`].
    pub written: &'a [String],
    /// `-fclang-compat=22`.
    pub v22: bool,
}

/// Parses a `target("...")` attribute string (see
/// [`CodegenOptions::target_attr`]).
pub type TargetAttrParser = fn(&str, &TargetAttrModule<'_>) -> TargetAttrFeatures;

impl CodegenOptions {
    /// The IR will be optimized (`-O1` and above): definitions are
    /// emitted without the `-O0` `noinline optnone` pair (clang
    /// `CodeGenOpts.OptimizationLevel > 0`; spec/codegen.md §7p).
    #[must_use]
    pub fn optimize(&self) -> bool {
        self.opt_level > 0
    }
}

impl Default for CodegenOptions {
    fn default() -> Self {
        CodegenOptions {
            source_filename: String::new(),
            exceptions: true,
            rtti: true,
            ident: String::new(),
            outline_atomics: Some(true),
            target_features: Vec::new(),
            target_cpu_features: Vec::new(),
            target_features_written: Vec::new(),
            target_attr: None,
            sve: false,
            sve_vector_bits: None,
            complex_range: ComplexRange::Full,
            fp: kiln_ast::fp::FpOptions::default(),
            fp_denormal: None,
            opt_level: 0,
            opt_size: 0,
            strict_aliasing: true,
            clang_compat: 22,
            llvm_major: 19,
            omit_frame_pointer: None,
            omit_leaf_frame_pointer: None,
            auto_var_init: AutoVarInit::Uninitialized,
            auto_var_init_max_size: AUTO_VAR_INIT_MAX_SIZE,
            visibility: Visibility::Default,
            type_visibility: Visibility::Default,
            inlines_hidden: false,
            reloc: RelocModel::default(),
            semantic_interposition: None,
            direct_access_external_data: None,
            stack_protector: StackProtector::Off,
            unwind_tables: Some(UnwindTables::Async),
            no_plt: false,
            static_destructors: StaticDestructors::All,
            wrapv: false,
            wrapv_pointer: false,
            trapv: false,
            trapv_handler: None,
            null_pointer_is_valid: false,
            no_builtin: false,
            no_builtin_fns: Vec::new(),
            target_cpu: None,
            tune_cpu: None,
            no_inline: false,
            code_model: None,
            reflection_mangling: kiln_abi::ReflectionMangling::Gcc,
            outline_expansions: OUTLINE_EXPANSIONS_DEFAULT,
            contract_default_handler: true,
            cxx_modules: true,
            debug: DebugOptions::default(),
            coverage: CoverageOptions::default(),
            hardening: Hardening::default(),
            sanitize: SanitizeOptions::default(),
            ignorelist: None,
            lto: None,
            lto_module_flags: true,
        }
    }
}

/// A construct codegen does not support (yet), or an internal failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodegenError {
    /// Where the offending construct is, when known.
    pub loc: Option<Loc>,
    /// What went wrong.
    pub msg: String,
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

impl std::error::Error for CodegenError {}

/// Lowers a translation unit to an LLVM module.
///
/// `ast` must be free of errors. Constant expressions are folded with
/// `oven` first (which may cache values in `ast`). Warnings codegen
/// reports itself are dropped; see [`emit_with_diags`].
pub fn emit(
    ast: &mut Ast,
    oven: &mut Oven,
    target: &Target,
    opts: &CodegenOptions,
) -> Result<Module, CodegenError> {
    emit_with_diags(ast, oven, target, opts).0
}

/// [`emit`], also returning the warnings codegen reports itself (clang
/// reports them from `CodeGenModule`, e.g. `-Watomic-alignment`), in
/// emission order, for the caller to emit to its engine.
pub fn emit_with_diags(
    ast: &mut Ast,
    oven: &mut Oven,
    target: &Target,
    opts: &CodegenOptions,
) -> (Result<Module, CodegenError>, Vec<kiln_diag::Diag>) {
    emit_with_debug(ast, oven, target, opts, None)
}

/// [`emit_with_diags`] with the unit's sources, from which debug
/// information takes its files and lines (`opts.debug`; without `sm` no
/// debug information is emitted).
pub fn emit_with_debug(
    ast: &mut Ast,
    oven: &mut Oven,
    target: &Target,
    opts: &CodegenOptions,
    sm: Option<&kiln_source::SourceManager>,
) -> (Result<Module, CodegenError>, Vec<kiln_diag::Diag>) {
    let folds = fold::prepass(ast, oven, target);
    // The mangler reads data member descriptions from the AST
    // (spec/reflection.md §8.3).
    if oven.dms_count() > 0 {
        let dms = (1..=oven.dms_count())
            .filter_map(|i| u32::try_from(i).ok().and_then(kiln_ast::ids::DmsId::try_new))
            .filter_map(|id| oven.dms(id))
            .map(|d| kiln_ast::reflect::DmsDesc {
                ty: d.ty,
                name: d.name,
                alignment: d.alignment,
                bit_width: d.bit_width,
                nua: d.nua,
                annotations: oven.dms_annots(d.annots).to_vec(),
            })
            .collect();
        ast.reflections.set_dms(dms);
    }
    let void_ptr = ast.types.pointer_to(kiln_ast::QualType::VOID);
    let mut cg = module::Cg::new(&*ast, &folds, target, opts, void_ptr);
    cg.sm = sm;
    if let Some(sm) = sm {
        cg.start_debug_info(sm);
    }
    cg.run()
}

#[cfg(test)]
mod tests {
    use super::FramePointer::{self, All, NonLeaf, None as Omit};

    /// clang 19 and 22's `"frame-pointer"` for each target, `-O0`/`-O1+`
    /// and flag combination (probed with `--target=` each Linux triple;
    /// spec/codegen.md "Frame pointer").
    #[test]
    fn frame_pointer_matches_clang() {
        let r = FramePointer::resolve;
        // (x86-64, optimize, -f[no-]omit, -m[no-]omit-leaf) -> kind.
        let rows = [
            (true, false, None, None, All),
            (true, true, None, None, Omit),
            (true, true, Some(false), None, All),
            (true, false, Some(true), None, Omit),
            (true, false, None, Some(true), NonLeaf),
            (true, true, None, Some(true), Omit),
            (true, true, None, Some(false), Omit),
            (true, true, Some(false), Some(true), NonLeaf),
            (true, false, Some(true), Some(false), Omit),
            (false, false, None, None, NonLeaf),
            (false, true, None, None, NonLeaf),
            (false, true, Some(false), None, NonLeaf),
            (false, true, Some(true), None, Omit),
            (false, false, None, Some(false), All),
            (false, true, Some(true), Some(false), Omit),
        ];
        for (x86, opt, omit, leaf, want) in rows {
            assert_eq!(r(x86, opt, omit, leaf), want, "{x86} {opt} {omit:?} {leaf:?}");
        }
        assert_eq!((All.attr(), All.module_flag()), (Some("all"), Some(2)));
        assert_eq!((NonLeaf.attr(), NonLeaf.module_flag()), (Some("non-leaf"), Some(1)));
        assert_eq!((Omit.attr(), Omit.module_flag()), (None, None));
    }
}
