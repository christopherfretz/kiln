//! The attribute triage table (spec/sema.md §4.5, WP-805): every
//! attribute clang knows that kiln does not implement, with its class and
//! the reason that class is safe.
//!
//! - Class A changes semantics, the ABI or layout. kiln rejects it with
//!   the error `'X' attribute is not supported by kiln yet` rather than
//!   compile the program differently from clang. The error is not
//!   suppressed in system headers or by `-w`, and `__has_attribute`
//!   reports 0 for it.
//! - Class B only steers optimization, instrumentation, debug info or
//!   hardening. Dropping it is a valid refinement, so kiln accepts it
//!   silently.
//! - Class C only feeds diagnostics or analyses kiln does not run, or is
//!   one clang itself ignores for kiln's targets. Accepted silently.
//!
//! A name clang does not know at all keeps the `unknown attribute`
//! warning. Implemented attributes never reach the table: the caller
//! tries [`Support::Implemented`] spellings first.
//!
//! Each row lists only the spellings clang knows and kiln does not
//! implement, per target and clang release; the driver's coverage test
//! checks the rows against clang's `__has_attribute` tables.

use kiln_ast::attr::{AttrKind, AttrSyntax};

/// The class of an unimplemented clang attribute (module docs).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    /// Semantics, ABI or layout: an error until implemented.
    A,
    /// Optimization, instrumentation, debug or hardening: ignored.
    B,
    /// Diagnostics or analysis only: ignored.
    C,
}

/// How kiln treats one written attribute.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Support {
    /// kiln implements it as this kind.
    Implemented(AttrKind),
    /// clang knows it; it is class B or C and accepted silently.
    Ignored(Class),
    /// clang knows it; it is class A and kiln rejects it.
    Unsupported,
    /// clang does not know it (the `unknown attribute` warning).
    Unknown,
}

/// `__attribute__((x))`.
pub const G: u8 = 1;
/// `[[gnu::x]]`.
pub const GS: u8 = 2;
/// `[[clang::x]]`.
pub const CL: u8 = 4;
/// `[[x]]`.
pub const STD: u8 = 8;
/// `[[gsl::x]]`.
pub const GSL: u8 = 16;
/// `[[msvc::x]]`.
pub const MS: u8 = 32;
/// `[[omp::x]]`.
pub const OMP: u8 = 64;

/// One table row.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    /// The name without its `__` affixes.
    pub name: &'static str,
    /// The unimplemented spellings clang knows: aarch64 at clang 19,
    /// aarch64 at 22, x86-64 at 19, x86-64 at 22.
    pub spellings: [u8; 4],
    /// The class.
    pub class: Class,
    /// Why the class is right (class A: the effect kiln would drop, and
    /// the work package that implements it, if one is assigned).
    pub why: &'static str,
}

impl Row {
    /// The spellings clang `compat` knows on x86-64 (`x86`) or aarch64.
    pub fn spellings_at(&self, compat: u32, x86: bool) -> u8 {
        self.spellings[usize::from(x86) * 2 + usize::from(compat >= 22)]
    }
}

/// Same spellings on both targets and both clang releases.
const fn r(name: &'static str, sp: u8, class: Class, why: &'static str) -> Row {
    Row { name, spellings: [sp; 4], class, why }
}

/// New in clang 22, on both targets.
const fn r22(name: &'static str, sp: u8, class: Class, why: &'static str) -> Row {
    Row { name, spellings: [0, sp, 0, sp], class, why }
}

/// Spellings that differ by target or release.
const fn rt(name: &'static str, spellings: [u8; 4], class: Class, why: &'static str) -> Row {
    Row { name, spellings, class, why }
}

use Class::{A, B, C};

const TSA: &str = "thread-safety analysis (-Wthread-safety), which kiln does not run";
const SAN: &str = "sanitizer instrumentation, which kiln does not emit";
const LIFETIME: &str = "lifetime analysis (-Wdangling), which kiln does not run";
const ANALYSIS: &str = "diagnostics or static analysis only";
const OBJC: &str = "Objective-C / CoreFoundation only; no effect on C++ code generation";
const SWIFT: &str = "Swift interop naming; no effect on C++ code generation";
const OFFLOAD: &str = "CUDA/HIP/OpenCL/SYCL/GPU only; clang ignores it for kiln's host targets";
const DEBUG: &str = "debug info only";
const INSTR: &str = "instrumentation or hardening only; dropping it changes no semantics";
const HINT: &str = "optimization hint; dropping it is a valid refinement";
const INLINE: &str = "inlining or call-optimization hint; dropping it changes no semantics";
const NOCC: &str = "calling convention clang ignores on aarch64 and x86-64";
const DIAG_REJECT: &str = "only makes clang diagnose a use; no code generation effect";

/// The rows, sorted by name.
pub static TABLE: &[Row] = &[
    r("NSObject", G | CL, C, OBJC),
    r("Owner", GSL, C, LIFETIME),
    r("Pointer", GSL, C, LIFETIME),
    r("aarch64_sve_pcs", G | CL, A, "calling convention: changes the call ABI (WP-808)"),
    r("aarch64_vector_pcs", G | CL, A, "calling convention (WP-808); an AArch64 declaration is accepted"),
    r("acquire_capability", G | CL, C, TSA),
    r("acquire_handle", G | CL, C, ANALYSIS),
    r("acquire_shared_capability", G | CL, C, TSA),
    r("acquired_after", G, C, TSA),
    r("acquired_before", G, C, TSA),
    r("address_space", G | CL, A, "places the pointee in another address space"),
    r("align_value", G, B, HINT),
    r("allocating", G | CL, C, ANALYSIS),
    r("always_inline", CL, B, INLINE),
    r("amdgpu_flat_work_group_size", G | CL, C, OFFLOAD),
    r("amdgpu_kernel", G | CL, C, OFFLOAD),
    r("amdgpu_max_num_work_groups", G | CL, C, OFFLOAD),
    r("amdgpu_num_sgpr", G | CL, C, OFFLOAD),
    r("amdgpu_num_vgpr", G | CL, C, OFFLOAD),
    r("amdgpu_waves_per_eu", G | CL, C, OFFLOAD),
    r("annotate", G | CL, C, ANALYSIS),
    r("annotate_type", CL, C, ANALYSIS),
    r("argument_with_type_tag", G | CL, C, ANALYSIS),
    r("arm_sve_vector_bits", G, A, "defines a vector or matrix type"),
    r("assert_capability", G | CL, C, TSA),
    r("assert_exclusive_lock", G, C, TSA),
    r("assert_shared_capability", G | CL, C, TSA),
    r("assert_shared_lock", G, C, TSA),
    r("assume", G | CL | OMP, B, HINT),
    r("assume_aligned", G | GS, B, HINT),
    r22("atomic", G | CL, B, HINT),
    r("availability", G | CL, C, ANALYSIS),
    r("available_only_in_default_eval_method", G | CL, C, ANALYSIS),
    r("blocking", G | CL, C, ANALYSIS),
    r("blocks", G | CL, C, OBJC),
    r("bounded", G, C, OFFLOAD),
    r("btf_decl_tag", G | CL, B, DEBUG),
    r("btf_type_tag", G | CL, B, DEBUG),
    r("builtin_alias", CL, A, "maps the declaration to a builtin"),
    r("callable_when", G | CL, C, ANALYSIS),
    r("callback", G | CL, B, HINT),
    r("called_once", G | CL, C, ANALYSIS),
    r("capability", G | CL, C, TSA),
    r("carries_dependency", G, C, ANALYSIS),
    r("cdecl", G | GS, C, NOCC),
    r("cf_audited_transfer", G | CL, C, OBJC),
    r("cf_consumed", G | CL, C, OBJC),
    r("cf_returns_not_retained", G | CL, C, OBJC),
    r("cf_returns_retained", G | CL, C, OBJC),
    r("cf_unknown_transfer", G | CL, C, OBJC),
    r("cfi_canonical_jump_table", G | CL, B, INSTR),
    r22("cfi_salt", G | CL, B, INSTR),
    r22("cfi_unchecked_callee", G | CL, B, INSTR),
    rt("clang_arm_builtin_alias", [G | CL, G | CL, 0, 0], A, "maps the declaration to a builtin"),
    r("clang_builtin_alias", G, A, "maps the declaration to a builtin"),
    r("cleanup", G | GS, A, "runs a cleanup function at scope exit (WP-807)"),
    r("clspv_libclc_builtin", G | CL, C, OFFLOAD),
    r22("cluster_dims", G, C, OFFLOAD),
    r("code_align", G | CL, B, HINT),
    r("common", G | GS, A, "common-symbol linkage (clang rejects it in C++)"),
    r("constant", G, C, OFFLOAD),
    r("constexpr", MS, C, OFFLOAD),
    r("constructor", G | GS, A, "registers the function in `llvm.global_ctors` / `llvm.global_dtors` (WP-807)"),
    r("consumable", G | CL, C, ANALYSIS),
    r("consumable_auto_cast_state", G | CL, C, ANALYSIS),
    r("consumable_set_state_on_read", G | CL, C, ANALYSIS),
    r("convergent", G | CL, B, INLINE),
    r22("coro_await_elidable", G | CL, B, HINT),
    r22("coro_await_elidable_argument", G | CL, B, HINT),
    r("coro_disable_lifetimebound", G | CL, C, LIFETIME),
    r("coro_lifetimebound", G | CL, C, LIFETIME),
    r("coro_return_type", G | CL, C, ANALYSIS),
    r("coro_wrapper", G | CL, C, ANALYSIS),
    r("counted_by", G | CL, C, ANALYSIS),
    r("counted_by_or_null", G | CL, C, ANALYSIS),
    r("cpu_dispatch", G | CL, A, "function multiversioning"),
    r("cpu_specific", G | CL, A, "function multiversioning"),
    r("cudart_builtin", G, C, OFFLOAD),
    r("destructor", G | GS, A, "registers the function in `llvm.global_ctors` / `llvm.global_dtors` (WP-807)"),
    r("device", G, C, OFFLOAD),
    r("device_builtin", G, C, OFFLOAD),
    r("device_builtin_surface_type", G, C, OFFLOAD),
    r("device_builtin_texture_type", G, C, OFFLOAD),
    r22("device_kernel", G | CL, C, OFFLOAD),
    r("diagnose_as_builtin", G | CL, C, ANALYSIS),
    r("disable_sanitizer_instrumentation", G | CL, B, INSTR),
    r("enforce_read_only_placement", G | CL, C, ANALYSIS),
    r("enforce_tcb", G | CL, C, ANALYSIS),
    r("enforce_tcb_leaf", G | CL, C, ANALYSIS),
    r("error", G | GS, C, DIAG_REJECT),
    r("exclusive_lock_function", G, C, TSA),
    r("exclusive_locks_required", G | CL, C, TSA),
    r("exclusive_trylock_function", G, C, TSA),
    r22("ext_vector_type", CL, A, "defines a vector type (only the GNU spelling is implemented)"),
    r("external_source_symbol", G | CL, C, ANALYSIS),
    r("fastcall", G | GS, C, NOCC),
    r("flatten", G | GS, B, INLINE),
    rt("force_align_arg_pointer", [0, 0, G | GS, G | GS], B, HINT),
    r("format_arg", G | GS, C, ANALYSIS),
    r22("format_matches", G | GS, C, ANALYSIS),
    rt("function_return", [0, 0, G | GS, G | GS], B, INSTR),
    r22("gcc_struct", G | GS, A, "selects the MS or GCC bit-field layout"),
    r("global", G, C, OFFLOAD),
    r22("grid_constant", G, C, OFFLOAD),
    r("guarded_by", G, C, TSA),
    r("guarded_var", G | CL, C, TSA),
    r("host", G, C, OFFLOAD),
    r("ibaction", G | CL, C, OBJC),
    r("iboutlet", G | CL, C, OBJC),
    r("iboutletcollection", G | CL, C, OBJC),
    r("intel_ocl_bicc", G | CL, A, "calling convention: changes the call ABI (WP-808)"),
    r("intel_reqd_sub_group_size", G, C, OFFLOAD),
    r("internal_linkage", G | CL, A, "gives the entity internal linkage (WP-807)"),
    rt("interrupt", [0, 0, 0, G | GS], A, "calling convention: changes the call ABI (WP-808)"),
    rt("kernel", [G, 0, G, 0], C, OFFLOAD),
    r("launch_bounds", G, C, OFFLOAD),
    r("leaf", G | GS, B, INLINE),
    r22("lifetime_capture_by", G | CL, C, LIFETIME),
    r("lifetimebound", G | CL, C, LIFETIME),
    r("loader_uninitialized", G | CL, B, HINT),
    r("lock_returned", G, C, TSA),
    r("lockable", G, C, TSA),
    r("locks_excluded", G, C, TSA),
    r("lto_visibility_public", G | CL, B, "whole-program devirtualization under LTO only"),
    r("m68k_rtd", G | CL, C, NOCC),
    r22("malloc_span", G | CL, B, HINT),
    r("managed", G, C, OFFLOAD),
    r("matrix_type", G | CL, A, "defines a vector or matrix type"),
    r("maybe_undef", G | CL, B, HINT),
    r("mig_server_routine", G | CL, C, ANALYSIS),
    rt("model", [0, 0, 0, G | GS], A, "x86-64 code model of the variable's section"),
    r22("modular_format", G | CL, B, HINT),
    r("ms_abi", G | GS, A, "calling convention: changes the call ABI (WP-808)"),
    r("ms_struct", G | GS, A, "selects the MS or GCC bit-field layout"),
    r("musttail", G | CL, A, "guaranteed tail call"),
    r("neon_polyvector_type", CL, A, "defines a vector type (only the GNU spelling is implemented)"),
    r("neon_vector_type", CL, A, "defines a vector type (only the GNU spelling is implemented)"),
    r("no_address_safety_analysis", G | GS, B, SAN),
    r("no_builtin", G | CL, A, "`no-builtins` / `no-builtin-X` function attributes (WP-808)"),
    rt("no_caller_saved_registers", [0, 0, G | GS, G | GS], A, "calling convention: changes the call ABI (WP-808)"),
    r22("no_cluster", G, C, OFFLOAD),
    r("no_instrument_function", G | GS, B, INSTR),
    r("no_profile_instrument_function", G | GS, B, INSTR),
    r("no_randomize_layout", G | GS, B, HINT),
    rt("no_sanitize", [G | CL, G | GS | CL, G | CL, G | GS | CL], B, SAN),
    r("no_sanitize_address", G | GS, B, SAN),
    r("no_sanitize_memory", G | CL, B, SAN),
    r("no_sanitize_thread", G | GS, B, SAN),
    r22("no_specializations", G | CL, C, ANALYSIS),
    r("no_speculative_load_hardening", G | CL, B, INSTR),
    r("no_split_stack", G | GS, B, INSTR),
    r("no_stack_protector", G | GS | CL, B, INSTR),
    r("no_thread_safety_analysis", G | CL, C, TSA),
    rt("nocf_check", [0, 0, G | GS, G | GS], B, INSTR),
    r("nocommon", G | GS, A, "common-symbol linkage (clang rejects it in C++)"),
    r22("noconvergent", G | CL, B, INLINE),
    r("noderef", G | CL, C, ANALYSIS),
    r("noduplicate", G | CL, B, INLINE),
    r("noescape", G | CL, B, HINT),
    r("noinline", CL | MS, B, INLINE),
    r("nomerge", G | CL, B, INLINE),
    r("nonallocating", G | CL, C, ANALYSIS),
    r("nonblocking", G | CL, C, ANALYSIS),
    r22("nonstring", G | GS, C, ANALYSIS),
    r("nosvm", G, C, OFFLOAD),
    r("not_tail_called", G | CL, B, INLINE),
    r("nouwtable", G | CL, B, INLINE),
    r("ns_consumed", G | CL, C, OBJC),
    r("ns_consumes_self", G | CL, C, OBJC),
    r("ns_error_domain", G, C, OBJC),
    r("ns_returns_autoreleased", G | CL, C, OBJC),
    r("ns_returns_not_retained", G | CL, C, OBJC),
    r("ns_returns_retained", G | CL, C, OBJC),
    r("nv_weak", G, C, OFFLOAD),
    r22("nvptx_kernel", G | CL, C, OFFLOAD),
    r("objc_arc_weak_reference_unavailable", G | CL, C, OBJC),
    r("objc_boxable", G | CL, C, OBJC),
    r("objc_bridge", G | CL, C, OBJC),
    r("objc_bridge_mutable", G | CL, C, OBJC),
    r("objc_bridge_related", G | CL, C, OBJC),
    r("objc_class_stub", G | CL, C, OBJC),
    r("objc_designated_initializer", G | CL, C, OBJC),
    r("objc_direct", G | CL, C, OBJC),
    r("objc_direct_members", G | CL, C, OBJC),
    r("objc_exception", G | CL, C, OBJC),
    r("objc_externally_retained", G | CL, C, OBJC),
    r("objc_gc", G | CL, C, OBJC),
    r("objc_independent_class", G | CL, C, OBJC),
    r("objc_method_family", G | CL, C, OBJC),
    r("objc_non_runtime_protocol", G | CL, C, OBJC),
    r("objc_nonlazy_class", G | CL, C, OBJC),
    r("objc_ownership", G | CL, C, OBJC),
    r("objc_precise_lifetime", G | CL, C, OBJC),
    r("objc_protocol_requires_explicit_implementation", G | CL, C, OBJC),
    r("objc_requires_property_definitions", G | CL, C, OBJC),
    r("objc_requires_super", G | CL, C, OBJC),
    r("objc_returns_inner_pointer", G | CL, C, OBJC),
    r("objc_root_class", G | CL, C, OBJC),
    r("objc_runtime_name", G | CL, C, OBJC),
    r("objc_runtime_visible", G | CL, C, OBJC),
    r("objc_subclassing_restricted", G | CL, C, OBJC),
    r("opencl_constant", G | CL, A, "places the pointee in another address space"),
    r("opencl_generic", G | CL, A, "places the pointee in another address space"),
    r("opencl_global", G | CL, A, "places the pointee in another address space"),
    r("opencl_global_device", G | CL, A, "places the pointee in another address space"),
    r("opencl_global_host", G | CL, A, "places the pointee in another address space"),
    r("opencl_local", G | CL, A, "places the pointee in another address space"),
    r("opencl_private", G | CL, A, "places the pointee in another address space"),
    r("opencl_unroll_hint", G, C, OFFLOAD),
    r("os_consumed", G | CL, C, OBJC),
    r("os_consumes_this", G | CL, C, OBJC),
    r("os_returns_not_retained", G | CL, C, OBJC),
    r("os_returns_retained", G | CL, C, OBJC),
    r("os_returns_retained_on_non_zero", G | CL, C, OBJC),
    r("os_returns_retained_on_zero", G | CL, C, OBJC),
    r("overloadable", G | CL, C, "no effect on C++ functions, which are already overloadable"),
    r("ownership_holds", G | CL, C, ANALYSIS),
    r("ownership_returns", G | CL, C, ANALYSIS),
    r("ownership_takes", G | CL, C, ANALYSIS),
    r("param_typestate", G | CL, C, ANALYSIS),
    r("pascal", G | CL, C, NOCC),
    r("pass_dynamic_object_size", G | CL, A, "hidden object-size parameter and `PS` mangling (WP-808)"),
    r("pass_object_size", G | CL, A, "hidden object-size parameter and `PS` mangling (WP-808)"),
    r("patchable_function_entry", G | GS, B, INSTR),
    r("pcs", G | GS, C, NOCC),
    r("pointer_with_type_tag", G | CL, C, ANALYSIS),
    r("preferred_type", G | CL, C, ANALYSIS),
    r("preserve_all", G | CL, A, "calling convention: changes the call ABI (WP-808)"),
    r("preserve_most", G | CL, A, "calling convention: changes the call ABI (WP-808)"),
    r("preserve_none", G | CL, A, "calling convention: changes the call ABI (WP-808)"),
    r("pt_guarded_by", G, C, TSA),
    r("pt_guarded_var", G | CL, C, TSA),
    r("ptrauth_vtable_pointer", G | CL, C, OFFLOAD),
    r("randomize_layout", G | GS, B, HINT),
    r22("reentrant_capability", G | CL, C, TSA),
    r("regcall", G | GS, A, "calling convention: changes the call ABI (WP-808)"),
    r("regparm", G | GS, C, NOCC),
    r("reinitializes", G | CL, C, ANALYSIS),
    r("release_capability", G | CL, C, TSA),
    r("release_generic_capability", G | CL, C, TSA),
    r("release_handle", G | CL, C, ANALYSIS),
    r("release_shared_capability", G | CL, C, TSA),
    r("reqd_work_group_size", G, C, OFFLOAD),
    r("require_constant_initialization", G | CL, C, ANALYSIS),
    r22("require_explicit_initialization", G | CL, C, ANALYSIS),
    r("requires_capability", G | CL, C, TSA),
    r("requires_shared_capability", G | CL, C, TSA),
    r("return_typestate", G | CL, C, ANALYSIS),
    r("returns_twice", G | GS, A, "marks calls `returns_twice` (WP-807)"),
    r("riscv_rvv_vector_bits", G, A, "defines a vector or matrix type"),
    r("scoped_lockable", G | CL, C, TSA),
    r("selectany", G | GS, A, "COMDAT / linkonce linkage for the variable"),
    r("sentinel", G | GS, C, ANALYSIS),
    r("set_typestate", G | CL, C, ANALYSIS),
    r("shared", G, C, OFFLOAD),
    r("shared_capability", G | CL, C, TSA),
    r("shared_lock_function", G, C, TSA),
    r("shared_locks_required", G | CL, C, TSA),
    r("shared_trylock_function", G, C, TSA),
    r("sized_by", G | CL, C, ANALYSIS),
    r("sized_by_or_null", G | CL, C, ANALYSIS),
    r("speculative_load_hardening", G | CL, B, INSTR),
    r("standalone_debug", G | CL, B, DEBUG),
    r("stdcall", G | GS, C, NOCC),
    r("suppress", G | CL | GSL, C, ANALYSIS),
    r("swift_async", G | CL, C, SWIFT),
    r("swift_async_context", G | CL, A, "Swift-convention parameter ABI (WP-808)"),
    r("swift_async_error", G | CL, C, SWIFT),
    r("swift_async_name", G, C, SWIFT),
    r("swift_attr", G, C, SWIFT),
    r("swift_bridge", G, C, SWIFT),
    r("swift_bridged_typedef", G, C, SWIFT),
    r("swift_context", G | CL, A, "Swift-convention parameter ABI (WP-808)"),
    r("swift_error", G, C, SWIFT),
    r("swift_error_result", G | CL, A, "Swift-convention parameter ABI (WP-808)"),
    r("swift_indirect_result", G | CL, A, "Swift-convention parameter ABI (WP-808)"),
    r("swift_name", G, C, SWIFT),
    r("swift_newtype", G, C, SWIFT),
    r("swift_objc_members", G, C, SWIFT),
    r("swift_private", G, C, SWIFT),
    r("swift_wrapper", G, C, SWIFT),
    r("swiftasynccall", G | CL, A, "calling convention: changes the call ABI (WP-808)"),
    r("swiftcall", G | CL, A, "calling convention: changes the call ABI (WP-808)"),
    r22("sycl_external", CL, C, OFFLOAD),
    r("sycl_kernel", G | CL, C, OFFLOAD),
    r22("sycl_kernel_entry_point", CL, C, OFFLOAD),
    r("sycl_special_class", G | CL, C, OFFLOAD),
    r("sysv_abi", G | GS, C, NOCC),
    r("target_clones", G | GS, A, "function multiversioning"),
    rt("target_version", [G | GS, G | GS, G | GS, 0], A, "function multiversioning"),
    r("test_typestate", G | CL, C, ANALYSIS),
    r("thiscall", G | GS, C, NOCC),
    r("tls_model", G | GS, A, "selects the TLS access model (WP-807)"),
    r("transparent_union", G | GS, A, "changes how the union is passed"),
    r("try_acquire_capability", G | CL, C, TSA),
    r("try_acquire_shared_capability", G | CL, C, TSA),
    r("type_tag_for_datatype", G | CL, C, ANALYSIS),
    r("unavailable", G | CL, C, DIAG_REJECT),
    r("unlock_function", G | CL, C, TSA),
    r("unsafe_buffer_usage", G | CL, C, ANALYSIS),
    r("use_handle", G | CL, C, ANALYSIS),
    r("vec_type_hint", G, C, OFFLOAD),
    r("vecreturn", G | CL, C, OFFLOAD),
    r("vectorcall", G | CL, A, "calling convention: changes the call ABI (WP-808)"),
    r("warn_unused", G | GS, C, ANALYSIS),
    r("warn_unused_result", CL, C, ANALYSIS),
    r("warning", G | GS, C, DIAG_REJECT),
    r("weak_import", G | CL, A, "`extern_weak` linkage on ELF"),
    r("work_group_size_hint", G, C, OFFLOAD),
    r("xray_always_instrument", G | CL, B, INSTR),
    r("xray_log_args", G | CL, B, INSTR),
    r("xray_never_instrument", G | CL, B, INSTR),
    r("zero_call_used_regs", G | GS, B, INSTR),
];

/// `[[clang::x]]` spellings kiln implements (clang 19 names only the
/// attribute, not its scope, in the unknown-attribute warning).
const CLANG_SCOPED: &[&str] = &[
    "always_destroy",
    "coro_only_destroy_when_complete",
    "disable_tail_calls",
    "enum_extensibility",
    "exclude_from_explicit_instantiation",
    "fallthrough",
    "flag_enum",
    "min_vector_width",
    "minsize",
    "no_destroy",
    "optnone",
    "preferred_name",
    "trivial_abi",
    "type_visibility",
    "uninitialized",
    "using_if_exists",
];

/// The table row for `name` (its `__` affixes removed).
pub fn row(name: &str) -> Option<&'static Row> {
    TABLE.binary_search_by(|r| r.name.cmp(name)).ok().map(|i| &TABLE[i])
}

/// The spelling bit of `syntax` with normalized scope `scope` (0 for a
/// keyword, `__declspec` or pragma attribute, and for a scope clang does
/// not know).
pub fn spelling_of(syntax: AttrSyntax, scope: Option<&str>) -> u8 {
    match (syntax, scope) {
        (AttrSyntax::Gnu, _) => G,
        (AttrSyntax::Cxx11, None) => STD,
        (AttrSyntax::Cxx11, Some("gnu")) => GS,
        (AttrSyntax::Cxx11, Some("clang")) => CL,
        (AttrSyntax::Cxx11, Some("gsl")) => GSL,
        (AttrSyntax::Cxx11, Some("msvc")) => MS,
        (AttrSyntax::Cxx11, Some("omp")) => OMP,
        _ => 0,
    }
}

/// How kiln treats attribute `name` (its `__` affixes removed) written
/// with `syntax` and normalized scope `scope`, under clang `compat`, in
/// C++26 or later (`cxx26`), on x86-64 (`x86`) or aarch64.
pub fn attribute_support(
    name: &str,
    syntax: AttrSyntax,
    scope: Option<&str>,
    compat: u32,
    cxx26: bool,
    x86: bool,
) -> Support {
    let clang_scope = scope == Some("clang");
    let implementable =
        scope.is_none_or(|s| s == "gnu") || (clang_scope && CLANG_SCOPED.contains(&name));
    if implementable {
        let gnu = syntax == AttrSyntax::Gnu || scope == Some("gnu");
        let std = scope.is_none() && syntax != AttrSyntax::Gnu;
        // [dcl.attr.indet] is C++26; P3475R2 removes `carries_dependency`
        // from C++26 (WP-743).
        if let Some(k) = crate::decl::attr_kind_of(name, std, gnu, clang_scope, syntax).filter(|&k| {
            (k != AttrKind::INDETERMINATE || cxx26) && (k != AttrKind::CARRIES_DEPENDENCY || !cxx26)
        }) {
            return Support::Implemented(k);
        }
    }
    let sp = spelling_of(syntax, scope);
    // Accepted on a declaration only; a definition is rejected
    // (AttrKind::AARCH64_VECTOR_PCS_DECL; Sema::act_on_finish_function_def).
    if name == "aarch64_vector_pcs" && !x86 && sp & (G | GS | CL) != 0 {
        return Support::Implemented(AttrKind::AARCH64_VECTOR_PCS_DECL);
    }
    match row(name) {
        Some(r) if sp != 0 && r.spellings_at(compat, x86) & sp != 0 => match r.class {
            A => Support::Unsupported,
            c => Support::Ignored(c),
        },
        _ => Support::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_sorted_unique_and_reasoned() {
        assert!(TABLE.windows(2).all(|w| w[0].name < w[1].name));
        for r in TABLE {
            assert!(!r.why.is_empty(), "{}", r.name);
            assert!(r.spellings.iter().any(|&s| s != 0), "{}", r.name);
            assert!(!r.name.starts_with("__") && !r.name.ends_with("__"), "{}", r.name);
        }
        assert!(CLANG_SCOPED.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn spellings_follow_clang() {
        let s = |n, syn, sc, c| attribute_support(n, syn, sc, c, true, false);
        let (g, x) = (AttrSyntax::Gnu, AttrSyntax::Cxx11);
        // The thread-safety family (formerly WP-606's table).
        assert_eq!(s("guarded_by", g, None, 19), Support::Ignored(C));
        assert_eq!(s("guarded_by", x, Some("clang"), 22), Support::Unknown);
        assert_eq!(s("capability", x, Some("clang"), 19), Support::Ignored(C));
        assert_eq!(s("capability", x, None, 22), Support::Unknown);
        assert_eq!(s("reentrant_capability", g, None, 19), Support::Unknown);
        assert_eq!(s("reentrant_capability", g, None, 22), Support::Ignored(C));
        // Sanitizer and lifetime attributes (formerly WP-667's table).
        assert_eq!(s("no_sanitize", g, None, 19), Support::Ignored(B));
        assert_eq!(s("no_sanitize", x, Some("clang"), 19), Support::Ignored(B));
        assert_eq!(s("no_sanitize", x, Some("gnu"), 19), Support::Unknown);
        assert_eq!(s("no_sanitize", x, Some("gnu"), 22), Support::Ignored(B));
        assert_eq!(s("lifetimebound", x, None, 22), Support::Unknown);
        assert_eq!(s("lifetimebound", x, Some("gnu"), 22), Support::Unknown);
        assert_eq!(s("lifetimebound", x, Some("clang"), 22), Support::Ignored(C));
        assert_eq!(s("no_sanitize_address", x, Some("gnu"), 19), Support::Ignored(B));
        assert_eq!(s("no_sanitize_address", x, Some("clang"), 22), Support::Unknown);
        // Class A: rejected.
        assert_eq!(s("cleanup", g, None, 22), Support::Unsupported);
        assert!(matches!(s("mode", x, Some("gnu"), 19), Support::Implemented(_)));
        assert_eq!(s("swiftcall", x, Some("clang"), 22), Support::Unsupported);
        // Declaration-only on AArch64 (AttrKind::AARCH64_VECTOR_PCS_DECL).
        assert!(matches!(s("aarch64_vector_pcs", g, None, 19), Support::Implemented(_)));
        assert_eq!(
            attribute_support("aarch64_vector_pcs", g, None, 19, true, true),
            Support::Unsupported
        );
        // Target- and release-dependent rows.
        assert_eq!(s("interrupt", g, None, 22), Support::Unknown);
        assert_eq!(attribute_support("interrupt", g, None, 22, true, true), Support::Unsupported);
        assert_eq!(s("kernel", g, None, 19), Support::Ignored(C));
        assert_eq!(s("kernel", g, None, 22), Support::Unknown);
        // Implemented spellings win; other spellings of the same name do not.
        assert!(matches!(s("noinline", g, None, 22), Support::Implemented(_)));
        assert_eq!(s("noinline", x, Some("clang"), 22), Support::Ignored(B));
        assert!(matches!(s("assume", x, None, 22), Support::Implemented(_)));
        assert_eq!(s("assume", g, None, 22), Support::Ignored(B));
        assert!(matches!(s("no_destroy", x, Some("clang"), 22), Support::Implemented(_)));
        assert!(matches!(s("retain", g, None, 19), Support::Implemented(_)));
        // C++26 drops `carries_dependency` (unknown, as clang warns).
        assert_eq!(s("carries_dependency", x, None, 22), Support::Unknown);
        assert!(matches!(
            attribute_support("carries_dependency", x, None, 22, false, false),
            Support::Implemented(_)
        ));
        assert_eq!(s("not_an_attribute", g, None, 22), Support::Unknown);
    }

    /// The names sibling work packages own stay class A until they land.
    #[test]
    fn owned_names_are_class_a() {
        for n in [
            "constructor",
            "destructor",
            "cleanup",
            "internal_linkage",
            "tls_model",
            "returns_twice",
            "no_builtin",
            "pass_object_size",
            "vectorcall",
            "ms_abi",
            "preserve_most",
        ] {
            assert_eq!(row(n).map(|r| r.class), Some(A), "{n}");
        }
    }
}
