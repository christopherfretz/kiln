// [temp.param]/10, [expr.prim.lambda.closure]/2: a lambda in a default
// template argument is a new closure type at each use. Like clang 19 and
// 22, kiln numbers it then in the template's namespace or class
// (spec/abi.md §8.15, WP-1062): `N::m()` is `N::m<N::{lambda()#1}>`
// (`_ZN1N1mINS_UlvE_EEEiv`) and the next use `N::m<N::{lambda()#2}>`
// (`UlvE0_`), `R::d()` numbers `R`'s closures (`UliE_`) and `S<int>::m()`
// those of `S<int>` (`NS0_UlvE_E`). All have the linkage of
// their namespace or class, so Inputs/lambda-default-arg-mangling-defs.cpp
// defines each under clang's name with an asm label and the program links
// only if kiln's references carry those names.
//@ std: c++20
//@ expect: run
//@ args: %{test_dir}/Inputs/lambda-default-arg-mangling-defs.cpp
//@ stdout: 1 2 3 4
extern "C" int printf(const char *, ...);
namespace N {
template <class U = decltype([] {})> int m();
}
struct R {
  template <class U = decltype([](int) {})> static int d();
};
template <class T> struct S {
  template <class U = decltype([] {})> static int m();
};
int main() { printf("%d %d %d %d\n", N::m(), N::m(), R::d(), S<int>::m()); }
