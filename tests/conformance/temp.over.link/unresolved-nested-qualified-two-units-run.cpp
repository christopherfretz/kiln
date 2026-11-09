// [temp.over.link]/5, [temp.dep.expr]: a non-member name with a nested
// qualifier in a dependent signature is an Itanium <unresolved-name>
// (spec/abi.md §8.15, WP-1061). clang 19 and 22 write the namespace levels
// as `sr <level>+ E` (`N::M::g(t)` is `sr1N1ME1g`; a namespace alias and
// an inline namespace are spelled as written, `NA::g` as `sr2NAE1g` and
// `N::I::q` as `sr1N1IE1q`) and a
// dependent type qualifier as `srN <type> <level>+ E` (`T::B::g(t)` is
// `srNT_1BE1g`). Inputs/unresolved-qualifier-defs.cpp defines each
// specialization under clang's name with an asm label, so the program
// links only if kiln's references carry those names.
//@ std: c++20
//@ expect: run
//@ args: %{test_dir}/Inputs/unresolved-qualifier-defs.cpp
//@ stdout: 5 6 7 8
extern "C" int printf(const char *, ...);
namespace N {
namespace M {
int g(int);
long g(long);
}
inline namespace I {
int q(int);
long q(long);
}
int g(int);
long g(long);
}
namespace NA = N;
struct A {
  struct B { static int g(A); };
};
template <class T> auto f5(T t) -> decltype(N::M::g(t));
template <class T> auto f6(T t) -> decltype(NA::g(t));
template <class T> auto f7(T t) -> decltype(N::I::q(t));
template <class T> auto f8(T t) -> decltype(T::B::g(t));
int main() { printf("%d %d %d %d\n", f5(1), f6(1), f7(1), f8(A())); }
