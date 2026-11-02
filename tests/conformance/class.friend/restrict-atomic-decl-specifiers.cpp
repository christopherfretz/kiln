// [class.friend], [dcl.type.general]: `restrict` is no keyword in C++, so
// `friend restrict int f();` names an unknown type ("unknown type name
// 'restrict'", the `int` after it dropped silently and the declaration
// invalid, as for any undeclared name before a type-specifier keyword, at
// namespace, class and block scope); `__restrict` on a non-pointer is
// "restrict requires a pointer or reference" and is dropped (C99 6.7.3p2);
// `_Atomic` on a friend function's return type is accepted. clang 19 and
// clang 22 agree.
// Verified against clang 19 and clang 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:10 error "unknown type name 'restrict'"
//@ diag: 26:10 error "restrict requires a pointer or reference ('int' is invalid)"
//@ diag: 27:10 error "restrict requires a pointer or reference ('int' is invalid)"
//@ diag: 29:10 error "unknown type name 'foo'"
//@ diag: 30:3 error "unknown type name 'restrict'"
//@ diag: 31:3 error "restrict requires a pointer or reference ('int' is invalid)"
//@ diag: 35:1 error "unknown type name 'restrict'"
//@ diag: 36:1 error "restrict requires a pointer or reference ('int' is invalid)"
//@ diag: 37:1 error "unknown type name 'foo'"
//@ diag: 39:3 error "unknown type name 'restrict'"
//@ diag: 40:3 error "unknown type name 'bar'"
struct S {
  friend restrict int f();
  friend _Atomic int g();
  friend __restrict int h();
  friend __restrict int *p();
  friend _Atomic(int) k();
  friend foo unsigned m();
  restrict int n();
  __restrict int o;
};
_Atomic int g();
_Atomic int g() { return 0; }
restrict int t;
__restrict int u;
foo const v = 1;
void q() {
  restrict int z;
  bar const w = 2;
  z = w;
}
