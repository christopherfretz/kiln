// [dcl.attr.grammar]/5 [dcl.fct] Calling-convention attributes on AArch64
// as clang 19 (`AArch64TargetInfo::checkCallingConvention`): `sysv_abi`
// and `cdecl` are the default convention, the x86 conventions and `pcs`
// warn and are ignored, `no_caller_saved_registers` and `interrupt` are
// unknown, `regparm` is an error, and `aarch64_vector_pcs` /
// `aarch64_sve_pcs` are checked as conventions. spec/types.md §5.4,
// WP-808.
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu
//@ strict-diags
//@ diag: 27:16 warning "'regcall' calling convention is not supported for this target"
//@ diag: 28:16 warning "'vectorcall' calling convention is not supported for this target"
//@ diag: 29:16 warning "'intel_ocl_bicc' calling convention is not supported for this target"
//@ diag: 30:16 warning "'thiscall' calling convention is not supported for this target"
//@ diag: 31:16 warning "'pcs' calling convention is not supported for this target"
//@ diag: 33:16 warning "unknown attribute 'no_caller_saved_registers' ignored"
//@ diag: 34:16 warning "unknown attribute 'interrupt' ignored"
//@ diag: 36:16 error "'regparm' is not valid on this platform"
//@ diag: 40:39 error "function declared 'aarch64_sve_pcs' here was previously declared 'aarch64_vector_pcs'"
//@ diag: 38:42 note "previous declaration is here"
//@ diag: 41:16 error "'aarch64_vector_pcs' attribute takes no arguments"
//@ diag: 42:16 warning "'preserve_none' only applies to function types; type here is 'int'"
//@ diag: 42:16 warning "'preserve_none' attribute only applies to functions and function pointers"
// Conventions AArch64 ignores warn and are dropped.
__attribute__((sysv_abi)) void a();
__attribute__((cdecl)) void b();
__attribute__((regcall)) void c();
__attribute__((vectorcall)) void d();
__attribute__((intel_ocl_bicc)) void e();
__attribute__((thiscall)) void f();
__attribute__((pcs("aapcs-vfp"))) void g();
// x86-only attributes are unknown here.
__attribute__((no_caller_saved_registers)) void h();
__attribute__((interrupt)) void i(void *);
// regparm is an error.
__attribute__((regparm(1))) void j();
// The AArch64 conventions and their checks.
__attribute__((aarch64_vector_pcs)) void k();
void k();
__attribute__((aarch64_sve_pcs)) void k();
__attribute__((aarch64_vector_pcs(1))) void l();
__attribute__((preserve_none)) int m;
// The default convention is not ignored: the redeclaration agrees.
void (*p)() = a;
