// [expr.prim.lambda.closure]/2, [over.over]: clang resolves the address
// of an explicit template-id more than once when it converts it to a
// target type (`IsStandardConversion` and `PerformImplicitConversion`
// each call `ResolveAddressOfOverloadedFunction`; `auto` deduction and
// argument passing resolve it once more), and every resolution runs the
// explicit-argument step, which numbers the trailing return type's lambda
// in the pattern (spec/sema-lambda.md §3.5). So `fq<int>` copy-initializing
// a pointer takes three numbers and `fq<long>`'s closure is `UlvE3_`;
// `auto p = &fp<int>` and `take(ft<int>)` take four (`UlvE4_`). Kiln
// resolves each once (two numbers), so Inputs/lambda-in-signature-
// explicit-args-address-defs.cpp, which calls clang's names, does not
// link. `pick` keeps `fq<int>` returning `int` so the address converts.
//@ std: c++20
//@ expect: run
//@ args: %{test_dir}/Inputs/lambda-in-signature-explicit-args-address-defs.cpp
//@ stdout: 3 3
//@ xfail(kiln): WP-1086 follow-up, overload/address.rs resolves an explicit template-id for a target once where clang re-runs the explicit-argument step per resolution pass
#include <cstdio>
template <class T> struct is_int { static constexpr bool v = false; };
template <> struct is_int<int> { static constexpr bool v = true; };
template <class T, class L> auto pick(L l) {
  if constexpr (is_int<T>::v) return 0; else return l;
}
template <class T> auto fq(T) -> decltype(pick<T>([] { return 1; })) { return {}; }
template <class T> auto fp(T) -> decltype(pick<T>([] { return 1; })) { return {}; }
template <class T> auto ft(T) -> decltype(pick<T>([] { return 1; })) { return {}; }
int take(int (*f)(int)) { return f(0); }
int other_address();
int main() {
  int (*q)(int) = fq<int>;
  auto p = &fp<int>;
  int r = q(0) + p(0) + take(ft<int>);
  std::printf("%d %d\n", r + fq<long>(1)() + fp<long>(1)() + ft<long>(1)(), other_address());
}
