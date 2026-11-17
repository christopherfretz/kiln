// [dcl.attr.grammar]/4: a keyword or an alternative token in an
// attribute-token (its name or its attribute-namespace, also the
// namespace of an attribute-using-prefix) is an identifier, so each of
// these is an unknown attribute, warned and ignored, not a parse error;
// balanced-token arguments are skipped. An attribute of a using-prefix
// starts at the prefix's namespace (clang 22). `[[alignas]]` is not the
// alignas specifier. Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
//@ diag: 27:3 warning "unknown attribute 'class' ignored"
//@ diag: 28:3 warning "unknown attribute 'x::new' ignored"
//@ diag: 29:3 warning "unknown attribute 'namespace::foo' ignored"
//@ diag: 30:3 warning "unknown attribute 'int' ignored"
//@ diag: 30:8 warning "unknown attribute 'true' ignored"
//@ diag: 31:3 warning "unknown attribute 'x::and' ignored"
//@ diag: 32:9 warning "unknown attribute 'if::else' ignored"
//@ diag: 33:3 warning "unknown attribute 'const::x' ignored"
//@ diag: 33:13 warning "unknown attribute 'x::const' ignored"
//@ diag: 34:3 warning "unknown attribute 'alignas' ignored"
//@ diag: 35:3 warning "unknown attribute 'gnu::volatile' ignored"
//@ diag: 39:11 warning "unknown attribute 'and::bitor' ignored"
//@ diag: 39:23 warning "unknown attribute 'bar::xor_eq' ignored"
//@ diag: 39:36 warning "unknown attribute 'compl::baz' ignored"
//@ diag: 39:51 warning "unknown attribute 'bitand::xor_eq' ignored"
[[class]] int a1;
[[x::new]] int a2;
[[namespace::foo]] int a3;
[[int, true]] int a4;
[[x::and(1, {2}, [3])]] int a5;
[[using if: else]] int a6;
[[const::x, x::const]] int a7;
[[alignas]] int a8;
[[gnu::volatile]] int a9;
[[using gnu: const, noreturn]] int f1();
[[gnu::const]] int f2();
int g() {
  int i [[and::bitor, bar::xor_eq, compl::baz(1), bitand::xor_eq(2, 3)]] = 0;
  return i;
}
