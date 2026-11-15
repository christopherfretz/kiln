// [ACLE] The target features the SME keyword attributes need, as clang
// 19: a streaming definition needs `sme`, ZA state `sme` and ZT0 state
// `sme2` (the function's own features, its `target` attribute included),
// and a call to a streaming function `sme` in the caller. With `sme` but
// not `sve`, SVE types are allowed in a streaming function only.
// spec/types.md §5.4, WP-1426.
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu
//@ strict-diags
//@ diag: 17:6 error "function executed in streaming-SVE mode requires 'sme'"
//@ diag: 18:30 error "function executed in streaming-SVE mode requires 'sme'"
//@ diag: 20:6 error "function using ZA state requires 'sme'"
//@ diag: 21:22 error "function using ZA state requires 'sme'"
//@ diag: 23:16 error "call to a streaming function requires 'sme'"
//@ diag: 24:6 error "function using ZT0 state requires 'sme2'"
//@ diag: 29:37 error "SVE vector type 'svbool_t' (aka '__SVBool_t') cannot be used in a non-streaming function"
void s1() __arm_streaming {}
__arm_locally_streaming void s2() {}
void s3() __arm_streaming_compatible {}
void s4() __arm_inout("za") {}
__arm_new("za") void s5() {}
void s6() __arm_streaming;
void s7() { s6(); }
void s8() __arm_inout("zt0") {}
__attribute__((target("sme"))) void s9() __arm_streaming {}
__attribute__((target("sme"))) void s10() { s6(); }
typedef __SVBool_t svbool_t;
__attribute__((target("sme"))) void s11(svbool_t) __arm_streaming {}
__attribute__((target("sme"))) void s12(svbool_t) {}
__attribute__((target("sme"))) __arm_locally_streaming void s13(svbool_t) {}
