// [expr.prim.lambda.closure]/2: a call with explicit template arguments
// substitutes the signature twice (clang `SubstituteExplicitTemplateArguments`
// substitutes a trailing return type too, then deduction finishes), and
// each substitution numbers the return type's lambda in the pattern: the
// first explicit specialization's closure is `UlvE0_`, the second's
// `UlvE2_` (spec/abi.md §8.15, WP-1077). Kiln substitutes only a leading
// return type with the explicit arguments, so it numbers them `UlvE_` and
// `UlvE0_`, and Inputs/lambda-in-signature-explicit-args-defs.cpp, which calls
// clang's names, does not link.
//@ std: c++20
//@ expect: run
//@ args: %{test_dir}/Inputs/lambda-in-signature-explicit-args-defs.cpp
//@ stdout: 2 2
//@ xfail(kiln): WP-1077 follow-up, a trailing return type is not substituted with the explicit template arguments, so its lambda takes one number too few
#include <cstdio>
template <class T> auto ret2() -> decltype([] { return 1; }) { return {}; }
int other_explicit();
int main() { std::printf("%d %d\n", ret2<int>()() + ret2<char>()(), other_explicit()); }
