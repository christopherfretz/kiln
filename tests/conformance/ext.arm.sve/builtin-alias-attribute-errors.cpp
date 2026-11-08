// [clang ext] `__clang_arm_builtin_alias(B)` / `clang_builtin_alias(B)`
// (clang AttrDocs "__clang_arm_builtin_alias"): the argument is one
// identifier naming an AArch64 SVE/SME builtin; the attribute applies only
// to functions; an aliased declaration has no definition and is called
// directly. spec/landed/codegen/WP-810.md (WP-810).
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sve
//@ strict-diags
//@ diag: 18:26 error "'__clang_arm_builtin_alias' attribute can only be applied to an ARM builtin"
//@ diag: 19:26 error "'__clang_arm_builtin_alias' attribute can only be applied to an ARM builtin"
//@ diag: 20:26 error "'__clang_arm_builtin_alias' attribute requires parameter 1 to be an identifier"
//@ diag: 21:26 error "'__clang_arm_builtin_alias' attribute takes one argument"
//@ diag: 23:26 error "'__clang_arm_builtin_alias' attribute takes one argument"
//@ diag: 24:22 error "'__clang_arm_builtin_alias' attribute only applies to functions"
//@ diag: 27:6 error "definition of builtin function 'f9'"
//@ diag: 28:22 error "builtin functions must be directly called"
void f1() __attribute__((__clang_arm_builtin_alias(__builtin_sve_svcntw)));
void f2() __attribute__((__clang_arm_builtin_alias(foo)));
void f3() __attribute__((__clang_arm_builtin_alias(__builtin_neon_vaddq_v)));
void f4() __attribute__((__clang_arm_builtin_alias("x")));
void f5() __attribute__((__clang_arm_builtin_alias));
void f6() __attribute__((clang_builtin_alias(__builtin_sve_svcntw)));
void f7() __attribute__((__clang_arm_builtin_alias(__builtin_sve_svcntw, 1)));
int v __attribute__((__clang_arm_builtin_alias(__builtin_sve_svcntw)));
[[clang::builtin_alias(__builtin_sve_svcntw)]] void f8();
void f9() __attribute__((__clang_arm_builtin_alias(__builtin_sve_svcntb)));
void f9() {}
void g() { auto p = &f1; f1(); }
