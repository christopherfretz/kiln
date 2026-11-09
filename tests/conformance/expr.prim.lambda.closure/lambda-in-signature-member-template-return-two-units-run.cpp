// [expr.prim.lambda.closure]/2, [temp.inst]/3: instantiating `S<int>`
// substitutes its member templates' signatures; a lambda there then
// belongs to `S<int>` and no longer depends on a template parameter
// (clang `LDK_NeverDependent`: unevaluated, in a non-dependent class,
// spec/sema-lambda.md §9), so `decltype` of it is the closure type and
// `S<int>::mret<int>` is `_ZN1SIiE4mretIiEENS0_UlvE_ET_` (spec/abi.md
// §8.15, WP-1141). The variants: a nested class template (`N<int>::f`), a
// variable template member (its type is not re-substituted, so its call
// operator is numbered in `S<int>`), captures (`mcap`, `mref`) and generic
// lambdas (`UlTL0__E_`: the closure's parameter keeps its depth).
// Inputs/lambda-in-signature-member-template-defs.cpp calls each of them
// under clang's name (an asm label), so the program links only if kiln
// defines the same names.
//@ std: c++20
//@ expect: run
//@ args: %{test_dir}/Inputs/lambda-in-signature-member-template-defs.cpp
//@ stdout: 5 7 8 2 3 | 5 7 8 2 3
#include <cstdio>
template <class T> struct S {
  template <class U> static auto mret(U) -> decltype([] { return 5; }) { return {}; }
  template <class U> static constexpr decltype([] { return 8; }) vt{};
  template <class U> static auto mgen(U) -> decltype([](auto x) { return x; }) { return {}; }
  template <class U> struct N {
    template <class V> static auto f(V) -> decltype([] { return 7; }) { return {}; }
  };
};
template <class T> struct C {
  template <class U> static auto mcap(U u) -> decltype([u] { return 5; }) * { return nullptr; }
  template <class U> static auto mref(U u) -> decltype([&u] { return 5; }) * { return nullptr; }
};
void other_members();
int main() {
  std::printf("%d %d %d %d %d | ", S<int>::mret(1)(), S<int>::N<int>::f(1)(), S<int>::vt<int>(),
              S<int>::mgen(1)(2), !C<int>::mcap(1) + !C<int>::mref(1) + 1);
  other_members();
}
