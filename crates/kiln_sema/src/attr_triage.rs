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
const LIFETIME: &str = "lifetime analysis (-Wdangling), which kiln does not run";
const ANALYSIS: &str = "diagnostics or static analysis only";
const OBJC: &str = "Objective-C / CoreFoundation only; no effect on C++ code generation";
const SWIFT: &str = "Swift interop naming; no effect on C++ code generation";
const OFFLOAD: &str = "CUDA/HIP/OpenCL/SYCL/GPU only; clang ignores it for kiln's host targets";
const DEBUG: &str = "debug info only";
const INSTR: &str = "instrumentation or hardening only; dropping it changes no semantics";
const HINT: &str = "optimization hint; dropping it is a valid refinement";
const INLINE: &str = "inlining or call-optimization hint; dropping it changes no semantics";
const DIAG_REJECT: &str = "only makes clang diagnose a use; no code generation effect";
const MULTIVERSION: &str =
    "function multiversioning, which needs resolvers and ifuncs on x86-64 (AArch64: WP-810)";

/// The rows, sorted by name.
pub static TABLE: &[Row] = &[
    r("NSObject", G | CL, C, OBJC),
    r("acquire_capability", G | CL, C, TSA),
    r("acquire_handle", G | CL, C, ANALYSIS),
    r("acquire_shared_capability", G | CL, C, TSA),
    r("acquired_after", G, C, TSA),
    r("acquired_before", G, C, TSA),
    r("address_space", G | CL, A, "places the pointee in another address space"),
    r("allocating", G | CL, C, ANALYSIS),
    r("amdgpu_flat_work_group_size", G | CL, C, OFFLOAD),
    r("amdgpu_kernel", G | CL, C, OFFLOAD),
    r("amdgpu_max_num_work_groups", G | CL, C, OFFLOAD),
    r("amdgpu_num_sgpr", G | CL, C, OFFLOAD),
    r("amdgpu_num_vgpr", G | CL, C, OFFLOAD),
    r("amdgpu_waves_per_eu", G | CL, C, OFFLOAD),
    r("annotate", G | CL, C, ANALYSIS),
    r("annotate_type", CL, C, ANALYSIS),
    r("argument_with_type_tag", G | CL, C, ANALYSIS),
    r("assert_capability", G | CL, C, TSA),
    r("assert_exclusive_lock", G, C, TSA),
    r("assert_shared_capability", G | CL, C, TSA),
    r("assert_shared_lock", G, C, TSA),
    r22("atomic", G | CL, B, HINT),
    r("availability", G | CL, C, ANALYSIS),
    r("available_only_in_default_eval_method", G | CL, C, ANALYSIS),
    r("blocking", G | CL, C, ANALYSIS),
    r("blocks", G | CL, C, OBJC),
    r("bounded", G, C, OFFLOAD),
    r("btf_decl_tag", G | CL, B, DEBUG),
    r("btf_type_tag", G | CL, B, DEBUG),
    r("callable_when", G | CL, C, ANALYSIS),
    r("called_once", G | CL, C, ANALYSIS),
    r("capability", G | CL, C, TSA),
    r("carries_dependency", G, C, ANALYSIS),
    r("cf_audited_transfer", G | CL, C, OBJC),
    r("cf_consumed", G | CL, C, OBJC),
    r("cf_returns_not_retained", G | CL, C, OBJC),
    r("cf_returns_retained", G | CL, C, OBJC),
    r("cf_unknown_transfer", G | CL, C, OBJC),
    r22("cfi_salt", G | CL, B, INSTR),
    r22("cfi_unchecked_callee", G | CL, B, INSTR),
    r("clspv_libclc_builtin", G | CL, C, OFFLOAD),
    r22("cluster_dims", G, C, OFFLOAD),
    r("constant", G, C, OFFLOAD),
    r("constexpr", MS, C, OFFLOAD),
    r("consumable", G | CL, C, ANALYSIS),
    r("consumable_auto_cast_state", G | CL, C, ANALYSIS),
    r("consumable_set_state_on_read", G | CL, C, ANALYSIS),
    r("coro_disable_lifetimebound", G | CL, C, LIFETIME),
    r("coro_lifetimebound", G | CL, C, LIFETIME),
    r("coro_return_type", G | CL, C, ANALYSIS),
    r("coro_wrapper", G | CL, C, ANALYSIS),
    r("counted_by", G | CL, C, ANALYSIS),
    r("counted_by_or_null", G | CL, C, ANALYSIS),
    rt("cpu_dispatch", [0, 0, G | CL, G | CL], A, MULTIVERSION),
    rt("cpu_specific", [0, 0, G | CL, G | CL], A, MULTIVERSION),
    r("cudart_builtin", G, C, OFFLOAD),
    r("device", G, C, OFFLOAD),
    r("device_builtin", G, C, OFFLOAD),
    r("device_builtin_surface_type", G, C, OFFLOAD),
    r("device_builtin_texture_type", G, C, OFFLOAD),
    r22("device_kernel", G | CL, C, OFFLOAD),
    r("diagnose_as_builtin", G | CL, C, ANALYSIS),
    r("enforce_read_only_placement", G | CL, C, ANALYSIS),
    r("enforce_tcb", G | CL, C, ANALYSIS),
    r("enforce_tcb_leaf", G | CL, C, ANALYSIS),
    r("error", G | GS, C, DIAG_REJECT),
    r("exclusive_lock_function", G, C, TSA),
    r("exclusive_locks_required", G | CL, C, TSA),
    r("exclusive_trylock_function", G, C, TSA),
    r("external_source_symbol", G | CL, C, ANALYSIS),
    r("format_arg", G | GS, C, ANALYSIS),
    r22("format_matches", G | GS, C, ANALYSIS),
    r("global", G, C, OFFLOAD),
    r22("grid_constant", G, C, OFFLOAD),
    r("guarded_by", G, C, TSA),
    r("guarded_var", G | CL, C, TSA),
    r("host", G, C, OFFLOAD),
    r("ibaction", G | CL, C, OBJC),
    r("iboutlet", G | CL, C, OBJC),
    r("iboutletcollection", G | CL, C, OBJC),
    r("intel_reqd_sub_group_size", G, C, OFFLOAD),
    rt("interrupt", [0, 0, 0, G | GS], A, "calling convention: changes the call ABI (WP-808)"),
    rt("kernel", [G, 0, G, 0], C, OFFLOAD),
    r("launch_bounds", G, C, OFFLOAD),
    r22("lifetime_capture_by", G | CL, C, LIFETIME),
    r("lock_returned", G, C, TSA),
    r("lockable", G, C, TSA),
    r("locks_excluded", G, C, TSA),
    r22("malloc_span", G | CL, B, HINT),
    r("managed", G, C, OFFLOAD),
    r("matrix_type", G | CL, A, "defines a vector or matrix type"),
    r("mig_server_routine", G | CL, C, ANALYSIS),
    rt("model", [0, 0, 0, G | GS], A, "x86-64 code model of the variable's section"),
    r22("modular_format", G | CL, B, HINT),
    r22("no_cluster", G, C, OFFLOAD),
    r("no_randomize_layout", G | GS, B, HINT),
    r22("no_specializations", G | CL, C, ANALYSIS),
    r("no_thread_safety_analysis", G | CL, C, TSA),
    r("nocommon", G | GS, C, "a no-op in C++, whose variables are never common symbols"),
    r22("noconvergent", G | CL, B, INLINE),
    r("noderef", G | CL, C, ANALYSIS),
    r("noinline", MS, B, INLINE),
    r("nonallocating", G | CL, C, ANALYSIS),
    r("nonblocking", G | CL, C, ANALYSIS),
    r22("nonstring", G | GS, C, ANALYSIS),
    r("nosvm", G, C, OFFLOAD),
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
    r("ownership_holds", G | CL, C, ANALYSIS),
    r("ownership_returns", G | CL, C, ANALYSIS),
    r("ownership_takes", G | CL, C, ANALYSIS),
    r("param_typestate", G | CL, C, ANALYSIS),
    r("pointer_with_type_tag", G | CL, C, ANALYSIS),
    r("preferred_type", G | CL, C, ANALYSIS),
    r("pt_guarded_by", G, C, TSA),
    r("pt_guarded_var", G | CL, C, TSA),
    r("ptrauth_vtable_pointer", G | CL, C, OFFLOAD),
    r("randomize_layout", G | GS, B, HINT),
    r22("reentrant_capability", G | CL, C, TSA),
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
    r("riscv_rvv_vector_bits", G, A, "defines a vector or matrix type"),
    r("scoped_lockable", G | CL, C, TSA),
    r("sentinel", G | GS, C, ANALYSIS),
    r("set_typestate", G | CL, C, ANALYSIS),
    r("shared", G, C, OFFLOAD),
    r("shared_capability", G | CL, C, TSA),
    r("shared_lock_function", G, C, TSA),
    r("shared_locks_required", G | CL, C, TSA),
    r("shared_trylock_function", G, C, TSA),
    r("sized_by", G | CL, C, ANALYSIS),
    r("sized_by_or_null", G | CL, C, ANALYSIS),
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
    rt("target_clones", [0, 0, G | GS, G | GS], A, MULTIVERSION),
    rt("target_version", [0, 0, G | GS, 0], A, MULTIVERSION),
    r("test_typestate", G | CL, C, ANALYSIS),
    r("try_acquire_capability", G | CL, C, TSA),
    r("try_acquire_shared_capability", G | CL, C, TSA),
    r("type_tag_for_datatype", G | CL, C, ANALYSIS),
    r("unavailable", G | CL, C, DIAG_REJECT),
    r("unlock_function", G | CL, C, TSA),
    r("unsafe_buffer_usage", G | CL, C, ANALYSIS),
    r("use_handle", G | CL, C, ANALYSIS),
    r("vec_type_hint", G, C, OFFLOAD),
    r("vecreturn", G | CL, C, OFFLOAD),
    r("warn_unused", G | GS, C, ANALYSIS),
    r("warn_unused_result", CL, C, ANALYSIS),
    r("warning", G | GS, C, DIAG_REJECT),
    r("weak_import", G | CL, A, "`extern_weak` linkage on ELF"),
    r("work_group_size_hint", G, C, OFFLOAD),
];

/// `[[clang::x]]` spellings kiln implements (clang 19 names only the
/// attribute, not its scope, in the unknown-attribute warning).
const CLANG_SCOPED: &[&str] = &[
    "always_destroy",
    "always_inline",
    "assume",
    "coro_only_destroy_when_complete",
    "disable_tail_calls",
    "enum_extensibility",
    "exclude_from_explicit_instantiation",
    "ext_vector_type",
    "fallthrough",
    "flag_enum",
    "internal_linkage",
    "min_vector_width",
    "minsize",
    "musttail",
    "neon_polyvector_type",
    "neon_vector_type",
    "no_builtin",
    "no_destroy",
    "noinline",
    "optnone",
    "overloadable",
    "pass_dynamic_object_size",
    "pass_object_size",
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
        // from C++26 (WP-743). `gcc_struct` and `[[clang::ext_vector_type]]`
        // are new in clang 22 (WP-809).
        if let Some(k) =
            crate::decl::attr_kind_of(name, std, gnu, clang_scope, syntax).filter(|&k| {
                (k != AttrKind::INDETERMINATE || cxx26)
                    && (k != AttrKind::CARRIES_DEPENDENCY || !cxx26)
                    && (k != AttrKind::GCC_STRUCT || compat >= 22)
                    && (k != AttrKind::EXT_VECTOR_TYPE || !clang_scope || compat >= 22)
            })
        {
            return Support::Implemented(k);
        }
    }
    // The calling conventions, `regparm` and `no_caller_saved_registers`
    // (WP-808; `swiftcall` / `swiftasynccall` stay class A).
    if let Some(k) = crate::cconv::cc_attr_kind(name, syntax, scope, x86) {
        return Support::Implemented(k);
    }
    let sp = spelling_of(syntax, scope);
    // clang 19 handles x86 `interrupt` (`handleAnyX86InterruptAttr`)
    // although its `__has_attribute` answers 0 there: class A at every
    // release, not the unknown-attribute warning (WP-808 follow-up).
    if name == "interrupt" && x86 && sp & (G | GS) != 0 {
        return Support::Unsupported;
    }
    // WP-810: dispatched before the triage (`Sema::builtin_alias_attribute`,
    // `Sema::fmv_attribute`); the x86-64 multiversioning spellings are
    // class-A rows.
    let wp810 = match name {
        "builtin_alias" if sp == CL => Some(AttrKind::BUILTIN_ALIAS),
        "clang_builtin_alias" if sp == G => Some(AttrKind::BUILTIN_ALIAS),
        "__clang_arm_builtin_alias" if !x86 && sp & (G | CL) != 0 => Some(AttrKind::BUILTIN_ALIAS),
        "target_version" if !x86 && sp & (G | GS) != 0 => Some(AttrKind::TARGET_VERSION),
        "target_clones" if !x86 && sp & (G | GS) != 0 => Some(AttrKind::TARGET_CLONES),
        "cpu_dispatch" if !x86 && sp & (G | CL) != 0 => Some(AttrKind::CPU_DISPATCH),
        "cpu_specific" if !x86 && sp & (G | CL) != 0 => Some(AttrKind::CPU_SPECIFIC),
        _ => None,
    };
    if let Some(k) = wp810 {
        return Support::Implemented(k);
    }
    // WP-813: the class B attributes (`class_b_attrs.rs`).
    if let Some((k, _)) = crate::class_b_attrs::kind_of(name, syntax, scope, x86, compat) {
        return Support::Implemented(k);
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
        // WP-675 implements the sanitizer family (`class_b_attrs.rs`).
        let ns = Support::Implemented(AttrKind::NO_SANITIZE);
        assert_eq!(s("no_sanitize", g, None, 19), ns);
        assert_eq!(s("no_sanitize", x, Some("clang"), 19), ns);
        assert_eq!(s("no_sanitize", x, Some("gnu"), 19), Support::Unknown);
        assert_eq!(s("no_sanitize", x, Some("gnu"), 22), ns);
        assert_eq!(s("lifetimebound", x, None, 22), Support::Unknown);
        assert_eq!(s("lifetimebound", x, Some("gnu"), 22), Support::Unknown);
        assert_eq!(
            s("lifetimebound", x, Some("clang"), 22),
            Support::Implemented(AttrKind::LIFETIMEBOUND)
        );
        assert_eq!(s("Owner", x, Some("gsl"), 19), Support::Implemented(AttrKind::GSL_OWNER));
        assert_eq!(s("Pointer", x, None, 19), Support::Unknown);
        assert_eq!(s("Owner", g, None, 19), Support::Unknown);
        assert_eq!(s("no_sanitize_address", x, Some("gnu"), 19), ns);
        assert_eq!(s("no_sanitize_address", x, Some("clang"), 22), Support::Unknown);
        // Class A: rejected.
        assert_eq!(s("address_space", g, None, 22), Support::Unsupported);
        assert_eq!(s("address_space", x, Some("clang"), 19), Support::Unsupported);
        // Implemented by WP-809, some spellings only from clang 22.
        assert!(matches!(s("selectany", g, None, 22), Support::Implemented(_)));
        assert!(matches!(s("musttail", x, Some("clang"), 19), Support::Implemented(_)));
        assert!(matches!(s("neon_vector_type", x, Some("clang"), 19), Support::Implemented(_)));
        assert_eq!(s("gcc_struct", g, None, 19), Support::Unknown);
        assert!(matches!(s("gcc_struct", g, None, 22), Support::Implemented(_)));
        assert_eq!(s("ext_vector_type", x, Some("clang"), 19), Support::Unknown);
        assert!(matches!(s("ext_vector_type", x, Some("clang"), 22), Support::Implemented(_)));
        assert!(matches!(s("ext_vector_type", g, None, 19), Support::Implemented(_)));
        assert!(matches!(s("cleanup", g, None, 22), Support::Implemented(_)));
        assert!(matches!(s("mode", x, Some("gnu"), 19), Support::Implemented(_)));
        assert_eq!(s("swiftcall", x, Some("clang"), 22), Support::Unsupported);
        // The calling conventions (WP-808): each spelling clang 19 gives
        // it, on every target (one the target ignores warns, as clang).
        let cc = Support::Implemented(AttrKind::CALLING_CONV);
        assert_eq!(s("aarch64_vector_pcs", g, None, 19), cc);
        assert_eq!(attribute_support("aarch64_vector_pcs", g, None, 19, true, true), cc);
        assert_eq!(s("preserve_most", x, Some("clang"), 22), cc);
        assert_eq!(s("ms_abi", x, Some("gnu"), 19), cc);
        assert_eq!(s("ms_abi", x, Some("clang"), 19), Support::Unknown);
        assert_eq!(s("regparm", g, None, 22), Support::Implemented(AttrKind::REGPARM));
        assert_eq!(s("no_caller_saved_registers", g, None, 22), Support::Unknown);
        assert_eq!(
            attribute_support("no_caller_saved_registers", x, Some("gnu"), 19, true, true),
            Support::Implemented(AttrKind::NO_CALLER_SAVED_REGS)
        );
        assert!(matches!(s("no_builtin", x, Some("clang"), 19), Support::Implemented(_)));
        // Target- and release-dependent rows.
        assert_eq!(s("interrupt", g, None, 22), Support::Unknown);
        assert_eq!(attribute_support("interrupt", g, None, 22, true, true), Support::Unsupported);
        assert_eq!(attribute_support("interrupt", g, None, 19, true, true), Support::Unsupported);
        assert_eq!(s("kernel", g, None, 19), Support::Ignored(C));
        assert_eq!(s("kernel", g, None, 22), Support::Unknown);
        // Implemented spellings win; other spellings of the same name do not.
        assert!(matches!(s("noinline", g, None, 22), Support::Implemented(_)));
        assert!(matches!(s("noinline", x, Some("clang"), 22), Support::Implemented(_)));
        assert!(matches!(s("assume", x, None, 22), Support::Implemented(_)));
        assert!(matches!(s("assume", g, None, 22), Support::Implemented(_)));
        assert!(matches!(s("no_destroy", x, Some("clang"), 22), Support::Implemented(_)));
        assert!(matches!(s("retain", g, None, 19), Support::Implemented(_)));
        // C++26 drops `carries_dependency` (unknown, as clang warns).
        assert_eq!(s("carries_dependency", x, None, 22), Support::Unknown);
        assert!(matches!(
            attribute_support("carries_dependency", x, None, 22, false, false),
            Support::Implemented(_)
        ));
        assert_eq!(s("not_an_attribute", g, None, 22), Support::Unknown);
        // WP-810: implemented on AArch64; multiversioning is class A on x86-64.
        assert!(matches!(s("arm_sve_vector_bits", g, None, 22), Support::Implemented(_)));
        assert!(matches!(
            s("__clang_arm_builtin_alias", x, Some("clang"), 19),
            Support::Implemented(_)
        ));
        assert!(matches!(s("target_version", x, Some("gnu"), 22), Support::Implemented(_)));
        let x86 = |n, syn, sc, c| attribute_support(n, syn, sc, c, true, true);
        assert_eq!(x86("target_clones", g, None, 22), Support::Unsupported);
        assert_eq!(x86("target_version", g, None, 19), Support::Unsupported);
        assert_eq!(x86("target_version", g, None, 22), Support::Unknown);
        assert_eq!(x86("cpu_dispatch", x, Some("clang"), 19), Support::Unsupported);
        assert_eq!(x86("__clang_arm_builtin_alias", g, None, 19), Support::Unknown);
        assert!(matches!(x86("clang_builtin_alias", g, None, 19), Support::Implemented(_)));
    }

    /// The class B attributes WP-813 implements (`class_b_attrs.rs`).
    #[test]
    fn class_b_names_are_implemented() {
        for n in ["flatten", "no_stack_protector", "nomerge", "callback"] {
            let k = attribute_support(n, AttrSyntax::Gnu, None, 19, true, false);
            assert!(matches!(k, Support::Implemented(_)), "{n}");
        }
    }

    /// The conventions kiln does not lower yet stay class A (WP-808
    /// follow-ups).
    #[test]
    fn unlowered_conventions_are_class_a() {
        for n in ["swiftcall", "swiftasynccall", "interrupt"] {
            assert_eq!(row(n).map(|r| r.class), Some(A), "{n}");
        }
    }
}
