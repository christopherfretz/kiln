// [meta.reflection.result] reflect_constant, [temp.arg.nontype]/4: an
// uncaught throw when the template parameter object copied from the value
// is not template-argument-equivalent to it. The accepted side:
// reflect-constant-copy-equivalence.cpp.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ skip(gcc): GCC 16.1 throws for every class with a user-provided copy constructor
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:16 error "constexpr variable 'r' must be initialized by a constant expression"
//@ diag: 20:20 note "a template parameter object copied from a value of type 'A' is not template-argument-equivalent to it"
//@ diag: 20:20 error "call to consteval function 'std::meta::reflect_constant<A>' is not a constant expression"
//@ diag: 20:20 note "a template parameter object copied from a value of type 'A' is not template-argument-equivalent to it"
#include <meta>
struct A {
  int x;
  constexpr A(int p) : x(p) {}
  constexpr A(const A &o) : x(o.x ? o.x - 1 : o.x) {}
};
constexpr auto r = std::meta::reflect_constant(A(4));
