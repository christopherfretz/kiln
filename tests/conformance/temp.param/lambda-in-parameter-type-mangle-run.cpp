// [temp.param]/2, [expr.prim.lambda.closure]: a function template whose
// non-type template parameter type is decltype of a lambda call is valid and
// mangles the closure type in the template parameter's type
// (`_Z4nttpITnDTclL3$_0EEELi12EEiv` with clang). Found by WP-946's
// allowed-context tests.
//@ std: c++20
//@ xfail(kiln): WP-946 follow-up, a lambda expression in a template parameter type cannot be mangled ("cannot yet mangle expression type LambdaExpr")
//@ expect: run
//@ stdout: 12
#include <cstdio>
template<decltype([] { return 12; }()) N> int nttp() { return N; }
int main() { std::printf("%d\n", nttp<12>()); }
