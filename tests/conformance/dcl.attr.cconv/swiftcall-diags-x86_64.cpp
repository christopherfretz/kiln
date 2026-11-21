// The `swiftcall` / `swiftasynccall` attribute checks (clang
// `handleFunctionTypeAttr`): no arguments, a function type only, one
// convention per type, the same convention on every redeclaration and
// the function type keeps it (no conversion to a plain function
// pointer); a variadic prototype cannot use either (clang
// `supportsVariadicCall`, as for `regcall` and `vectorcall`).
// spec/sema.md §4.5, WP-1745 (WP-808).
//@ expect: compile-fail
//@ args: --target=x86_64-unknown-linux-gnu -std=c++17 -ferror-limit=0
//@ strict-diags
//@ diag: 22:26 error "'swiftcall' attribute takes no arguments"
//@ diag: 23:23 warning "'swiftcall' only applies to function types; type here is 'int'"
//@ diag: 24:53 error "ms_abi and swiftcall attributes are not compatible"
//@ diag: 26:6 error "function declared 'swiftasynccall' here was previously declared 'swiftcall'"
//@ diag: 25:6 note "previous declaration is here"
//@ diag: 29:8 error "cannot initialize a variable of type 'void (*)()' with an lvalue of type 'void () __attribute__((swiftcall))'"
//@ diag: 30:16 error "variadic function cannot use swiftasynccall calling convention"
//@ diag: 31:34 error "variadic function cannot use swiftcall calling convention"
//@ diag: 32:31 error "variadic function cannot use swiftasynccall calling convention"
//@ diag: 33:34 error "variadic function cannot use regcall calling convention"
//@ diag: 34:34 error "variadic function cannot use vectorcall calling convention"
void a1() __attribute__((swiftcall(1)));
int v1 __attribute__((swiftcall));
void a2() __attribute__((swiftcall)) __attribute__((ms_abi));
void a3() __attribute__((swiftcall));
void a3() __attribute__((swiftasynccall));
void a4() __attribute__((swiftcall));
void (*p1)() __attribute__((swiftcall)) = a4;
void (*p2)() = a4;
__attribute__((swiftasynccall)) void a5(...);
void a6(int, ...) __attribute__((swiftcall));
typedef void (*T)(int, ...) [[clang::swiftasynccall]];
void a7(int, ...) __attribute__((regcall));
void a8(int, ...) __attribute__((vectorcall));
[[clang::swiftasynccall]] void a9();
struct S { __attribute__((swiftcall)) void m(); };
