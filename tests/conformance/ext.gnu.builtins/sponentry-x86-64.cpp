// `__builtin_sponentry` is an AArch64 target builtin (clang
// BuiltinsAArch64.def): on x86-64 it is not declared, so naming it is an
// undeclared identifier and `__has_builtin` is false. Found in
// unwinder-style code. WP-1428.
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:11 error "use of undeclared identifier '__builtin_sponentry'"
#if __has_builtin(__builtin_sponentry)
#error __builtin_sponentry is an AArch64 builtin
#endif
void *s = __builtin_sponentry(0);
