// [expr.prim.lambda.closure]/2, [over.over]/1, /3, [temp.deduct.funcaddr]:
// `f<int>` completes `f<int, void>` from its explicit arguments, but the
// target deduces `U = int`: clang resolves the overload set at each step
// of the copy-initialization (three numbers for `f<int>` with its
// specialization) before `f<long>`'s deduction and specialization.
// Inputs/lambda-in-signature-explicit-args-mismatched-spec-defs.cpp calls
// `f<long>`'s closure under clang's name (`UlvE3_`), so the program
// links only if kiln numbers the lambda alike (spec/sema-lambda.md §3.5).
//@ std: c++20
//@ expect: run
//@ args: %{test_dir}/Inputs/lambda-in-signature-explicit-args-mismatched-spec-defs.cpp
//@ stdout: 1 1
//@ xfail(kiln): WP-1142 follow-up, the early specialization (U = void) is not the target's, and the retarget numbers each resolution twice
#include <cstdio>
template <class T> struct is_int { static constexpr bool v = false; };
template <> struct is_int<int> { static constexpr bool v = true; };
template <class T, class L> auto pick(L l) {
  if constexpr (is_int<T>::v) return 0; else return l;
}
template <class T, class U = void> auto f(U *, T) -> decltype(pick<T>([] { return 1; })) { return {}; }
using FP = int (*)(int *, int);
int other_mismatched();
int main() {
  FP q = f<int>;
  std::printf("%d %d\n", q(nullptr, 0) + f<long>((void *)nullptr, 1)(), other_mismatched());
}
