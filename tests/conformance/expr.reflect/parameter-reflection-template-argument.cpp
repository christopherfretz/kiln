// [temp.arg.nontype], [expr.const] (P2996R13): a template argument is
// manifestly constant-evaluated, so in a function template specialization
// `parameters_of(^^f<N>)[0]` as a template argument is one constant
// expression (the vector parameters_of returns is not a result of it),
// as it is outside a template. Found running GCC's C++ testsuite.
//@ xfail(kiln): WP-1613: substituted explicit template arguments are not a manifestly constant-evaluated context (templ/)
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
#include <meta>
template <std::meta::info I> void use() {}
template <int N> void f(int x) {
  use<std::meta::parameters_of(^^f<N>)[0]>();
}
template void f<1>(int);
