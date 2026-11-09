// [expr.prim.lambda.closure]/2, [temp.inst]/3: instantiating `S<int>`
// substitutes its member template `mret`'s return type, whose lambda then
// belongs to `S<int>` and no longer depends on a template parameter
// (clang `LDK_NeverDependent`: unevaluated, in a non-dependent class), so
// `decltype` of it is the closure type and `S<int>::mret<int>` is
// `_ZN1SIiE4mretIiEENS0_UlvE_ET_` (spec/abi.md §8.15, WP-1077). Kiln keeps
// the closure instantiation-dependent and mangles the return type as
// `DTLNS0_UlvE_EEE`, so Inputs/lambda-in-signature-member-template-defs.cpp, which
// calls clang's name, does not link.
//@ std: c++20
//@ expect: run
//@ args: %{test_dir}/Inputs/lambda-in-signature-member-template-defs.cpp
//@ stdout: 5 5
//@ xfail(kiln): WP-1077 follow-up, a lambda substituted into a member template's signature during class instantiation stays instantiation-dependent (decltype is not the closure type)
#include <cstdio>
template <class T> struct S {
  template <class U> static auto mret(U) -> decltype([] { return 5; }) { return {}; }
};
int other_member();
int main() { std::printf("%d %d\n", S<int>::mret(1)(), other_member()); }
