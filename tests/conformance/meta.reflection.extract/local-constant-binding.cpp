// [meta.reflection.extract]/12: extract by value of a block-scope
// structured binding of a constexpr object is
// `extract-value<U>(constant_of(r))`, and `[: r :]` of a structured
// binding whose scope encloses it is a valid splice-expression
// ([expr.prim.splice]/2.5), so this is a constant. GCC 16.1 rejects it
// ("not a valid argument to a splice-expression"): the text decides.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
#include <meta>

struct P { int a; int b; };

int g() {
  constexpr P p{4, 5};
  constexpr auto [u, v] = p;
  static_assert(std::meta::extract<int>(^^v) == 5);
  return u;
}
