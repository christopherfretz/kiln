// [class.copy.elision]/1: "copy elision is not permitted ... in constant
// initialization", so before C++17 `constexpr A b = A();` copies the
// temporary and `b.p` would point to it (not a constant expression).
// clang 22 elides the copy in constant evaluation even under
// -fno-elide-constructors, and so does kiln (docs/status/deviations.md);
// GCC 16.1 rejects with the flag only. Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -std=c++14 -fno-elide-constructors
//@ xfail(gcc): GCC 16.1 copies the temporary in constant initialization under -fno-elide-constructors

struct A {
  void *p;
  constexpr A() : p(this) {}
};
constexpr A a;
constexpr A b = A();
static_assert(a.p == &a, "");
static_assert(b.p == &b, "");
