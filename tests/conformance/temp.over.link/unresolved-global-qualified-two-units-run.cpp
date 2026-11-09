// [temp.over.link]/5, [temp.dep.expr]: a global-qualified non-member name
// in a dependent signature is an Itanium <unresolved-name> (spec/abi.md
// §8.15, WP-1061). clang 19 and 22 write `gs` before an unqualified
// global name (`::g(t)` is `clgs1gfp_E`) and before `sr` (`::N::g(t)` is
// `clgssr1NE1gfp_E`, `::N::h<int>(t)` is `clgssr1NE1hIiEfp_E`,
// `::N::X<T>::v` is `gssr1N1XIT_EE1v`). (An equivalent `N::g(t)` written
// earlier in the TU would lend `::N::g(t)` its spelling, `sr1NE1g`; see
// dependent-expr-first-spelling-two-units-run.cpp.)
// Inputs/unresolved-qualifier-defs.cpp defines each
// specialization under clang's name with an asm label, so the program
// links only if kiln's references carry those names.
//@ std: c++20
//@ expect: run
//@ args: %{test_dir}/Inputs/unresolved-qualifier-defs.cpp
//@ stdout: 1 2 3 4
extern "C" int printf(const char *, ...);
int g(int);
long g(long);
namespace N {
int g(int);
long g(long);
template <class T> int h(T);
template <class T> struct X { static constexpr int v = 0; };
}
template <class T> auto f1(T t) -> decltype(::g(t));
template <class T> auto f2(T t) -> decltype(::N::g(t));
template <class T> auto f3(T t) -> decltype(::N::h<int>(t));
template <class T> auto f4(T t) -> decltype(::N::X<T>::v + t);
int main() { printf("%d %d %d %d\n", f1(1), f2(1), f3(1), f4(1)); }
