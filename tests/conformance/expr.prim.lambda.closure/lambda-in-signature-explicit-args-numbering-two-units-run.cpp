// [expr.prim.lambda.closure]/2: a call with explicit template arguments
// substitutes the signature twice (clang `SubstituteExplicitTemplateArguments`
// substitutes a trailing return type too, after the parameter types, then
// deduction finishes), and each substitution numbers the return type's
// lambda in the pattern: the first explicit specialization's closure is
// `UlvE0_`, the second's `UlvE2_` (spec/sema-lambda.md §3.5, spec/abi.md
// §8.15). Before WP-1086 kiln substituted only a leading return type with
// the explicit arguments, numbered them `UlvE_` and `UlvE0_`, and
// Inputs/lambda-in-signature-explicit-args-defs.cpp, which calls clang's
// names, did not link. Every shape is in
// tests/mangle/lambda_in_signature_explicit_args.cpp.
//@ std: c++20
//@ expect: run
//@ args: %{test_dir}/Inputs/lambda-in-signature-explicit-args-defs.cpp
//@ stdout: 2 2
#include <cstdio>
template <class T> auto ret2() -> decltype([] { return 1; }) { return {}; }
int other_explicit();
int main() { std::printf("%d %d\n", ret2<int>()() + ret2<char>()(), other_explicit()); }
