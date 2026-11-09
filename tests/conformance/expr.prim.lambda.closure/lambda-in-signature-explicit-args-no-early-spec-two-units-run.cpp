// [expr.prim.lambda.closure]/2, [over.over]/1, /3, [temp.deduct.funcaddr]:
// `f<int>` leaves `U` undeduced, so it is an overload set only the target
// resolves: clang resolves it for the braced list's verifying pass, the
// implicit conversion sequence and the perform step (four numbers with
// the specialization) before `f<long>`'s deduction and specialization.
// Inputs/lambda-in-signature-explicit-args-no-early-spec-defs.cpp calls
// `f<long>`'s closure under clang's name (`UlvE4_`), so the program
// links only if kiln numbers the lambda alike (spec/sema-lambda.md §3.5).
//@ std: c++20
//@ expect: run
//@ args: %{test_dir}/Inputs/lambda-in-signature-explicit-args-no-early-spec-defs.cpp
//@ stdout: 1 1
//@ xfail(kiln): WP-1142 follow-up, no early specialization (U not deducible from the explicit arguments); the overload-set path resolves fewer times than clang
#include <cstdio>
template <class T> struct is_int { static constexpr bool v = false; };
template <> struct is_int<int> { static constexpr bool v = true; };
template <class T, class L> auto pick(L l) {
  if constexpr (is_int<T>::v) return 0; else return l;
}
template <class T, class U> auto f(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
using FP = int (*)(int *, int);
int other_noearly();
int main() {
  FP q{f<int>};
  std::printf("%d %d\n", q(nullptr, 0) + f<long>((void *)nullptr, 1)(), other_noearly());
}
