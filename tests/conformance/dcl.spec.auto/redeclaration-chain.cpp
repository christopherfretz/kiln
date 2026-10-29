// A redeclaration of a function with a deduced return type uses the
// placeholder, not the deduced type ([dcl.spec.auto.general]/13); the
// definition deduces it for every declaration, and a use before the
// definition is ill-formed ([dcl.spec.auto.general]/11).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:9 error "function 'g' with deduced return type cannot be used before it is defined"
//@ diag: 22:6 note "'g' declared here"
//@ diag: 26:5 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 25:6 note "previous definition is here"
//@ diag: 28:5 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 27:6 note "previous definition is here"
//@ diag: 30:6 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 29:6 note "previous declaration is here"
auto f();
auto f() { return 1; }
auto f();
static_assert(__is_same(decltype(f()), int));
decltype(auto) h();
decltype(auto) h() { return 'c'; }
decltype(auto) h();
auto g();
int x = g();
static_assert(__is_same(decltype(h()), char));
auto k() { return 2L; }
int k();
auto k2() { return 1; }
int k2();
auto n();
auto n() -> int;
auto m() { return 1; }
auto m();
auto m();
static_assert(__is_same(decltype(m()), int));
struct S {
  auto q();
  static auto r();
};
auto S::q() { return 1.0; }
auto S::r() { return 1u; }
static_assert(__is_same(decltype(S::r()), unsigned));
