// [temp.param]/2, [expr.prim.lambda.closure]: a function template whose
// non-type template parameter type is decltype of a lambda call is valid and
// mangles the closure type in the template parameter's type as clang does,
// `L <closure type> E` with the unnumbered closure `$_0`
// (`_Z4nttpITnDTclL3$_0EEELi12EEiv`; spec/abi.md §8.15, WP-1062). Found by
// WP-946's allowed-context tests.
//@ std: c++20
//@ expect: run
//@ stdout: 12
#include <cstdio>
template<decltype([] { return 12; }()) N> int nttp() { return N; }
int main() { std::printf("%d\n", nttp<12>()); }
