// P2686R5 [expr.const]/22 (C++26): a constexpr local is
// constexpr-referenceable from its own initializer, so a pointer to it in
// its value is a permitted result (`constexpr A a = f();` where
// `a.self == &a`), as is a pointer to another constexpr local; clang 22
// and GCC 16.1 do not implement this yet (spec/oven.md). Found running
// GCC's C++ testsuite (WP-1468).
//@ std: c++26
//@ requires: clang>=23
//@ expect: compile-ok
struct A { const A *self = this; };
constexpr A make() { return {}; }
int f() {
  constexpr int i = 42;
  constexpr const int *p = &i;
  constexpr A a = make();
  static_assert(a.self == &a && *p == 42);
  return *p;
}
