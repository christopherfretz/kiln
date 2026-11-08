// GNU `constructor` / `destructor` checks in clang's order (WP-807;
// clang `handleConstructorAttr`, `checkUInt32Argument`): the subject (a
// function, else a warning), then at most one argument, then a 32-bit
// integer constant. Any 32-bit value is accepted (no range check, unlike
// `init_priority`), as is a non-void return type or parameters.
// clang 22 evaluates a value-dependent priority at instantiation and
// names `[[clang::constructor]]` with its scope; the clang 19 behaviour
// is in constructor-diags-clang19.cpp.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:16 error "'constructor' attribute takes no more than 1 argument"
//@ diag: 26:16 error "'constructor' attribute requires an integer constant"
//@ diag: 29:16 warning "'constructor' attribute only applies to functions"
//@ diag: 30:16 warning "'destructor' attribute only applies to functions"
//@ diag: 38:16 error "'constructor' attribute requires an integer constant"
//@ diag: 39:16 error "'destructor' attribute requires an integer constant"
//@ diag: 40:28 error "integer constant expression evaluates to value 4294967296 that cannot be represented in a 32-bit unsigned integer type"
//@ diag: 41:16 warning "'constructor' attribute only applies to functions"
//@ diag: 42:29 warning "'destructor' attribute only applies to functions"
//@ diag: 43:3 warning "unknown attribute 'clang::constructor' ignored"
__attribute__((constructor(50))) void a();
__attribute__((constructor(0))) void a0();
__attribute__((constructor(-1))) void a2();
__attribute__((constructor(1, 2))) void a3();
__attribute__((constructor("x"))) void a4();
__attribute__((constructor)) int b() { return 0; }
__attribute__((constructor)) void c(int) {}
__attribute__((constructor)) int v;
__attribute__((destructor(1, 2))) int w;
struct S {
  __attribute__((constructor)) void m();
  __attribute__((destructor(7))) static void sm();
};
template <int N> __attribute__((constructor(N))) void tn() {}
template <int N> struct X { __attribute__((destructor(N))) static void f() {} };
int n = 200;
__attribute__((constructor(n))) void g();
__attribute__((destructor(101.0))) void h();
__attribute__((constructor(4294967296))) void big();
__attribute__((constructor)) void (*p)();
void loc() { __attribute__((destructor)) int x; (void)x; }
[[clang::constructor]] void q();
