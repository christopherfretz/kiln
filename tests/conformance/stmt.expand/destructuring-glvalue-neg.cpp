// [stmt.expand]/5.3 with [dcl.struct.bind]: the destructuring form is
// `constexpr auto&& [u...] = expansion-initializer;` for a constexpr
// for-range-declaration, so a glvalue range is bound by a constexpr
// reference and never copied. Before C++26 (P2686) only an object with
// static storage duration can be so referenced: an automatic one is an
// error even when it is constexpr or has no elements (CWG3048), as in
// GCC 16.1 (found running GCC's C++ testsuite).
//@ expect: compile-fail
//@ std: c++23
//@ strict-diags
//@ requires: clang>=23
//@ oracle: gcc
//@ diag: 28:3 warning "expansion statements are a C++26 extension"
//@ diag: 30:3 warning "expansion statements are a C++26 extension"
//@ diag: 32:3 warning "expansion statements are a C++26 extension"
//@ diag: 32:34 error "constexpr variable '[__u]' must be initialized by a constant expression"
//@ diag: 32:34 note "reference to 'a' is not a constant expression"
//@ diag: 31:5 note "declared here"
//@ diag: 34:3 warning "expansion statements are a C++26 extension"
//@ diag: 34:34 error "constexpr variable '[__u]' must be initialized by a constant expression"
//@ diag: 34:34 note "reference to 'b' is not a constant expression"
//@ diag: 33:15 note "address of non-static constexpr variable 'b' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
struct A {};
struct B { int x = 1, y = 2; };
void g(int);
void f() {
  static constexpr A sa{};
  template for (constexpr auto x : sa) { g(x); }
  static constexpr B sb{};
  template for (constexpr auto x : sb) { g(x); }
  A a{};
  template for (constexpr auto x : a) { g(x); }
  constexpr B b{};
  template for (constexpr auto x : b) { g(x); }
}
