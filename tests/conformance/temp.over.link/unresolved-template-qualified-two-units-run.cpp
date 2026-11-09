// [temp.over.link]/5, [temp.dep.expr]: a non-member name qualified by a
// template specialization in a dependent signature is an Itanium
// <unresolved-name> (spec/abi.md §8.15, WP-1061). clang 19 and 22 close
// the qualifier levels with `E` only after the last one (`N::X<T>::Y::g`
// is `sr1N1XIT_E1YE1g`, `N::X<T>::g` is `sr1N1XIT_EE1g`), spell a
// `template` member template as named (`T::template Z<int>::g` is
// `srNT_1ZIiEE1g`), keep an alias template's name (`AX<T>::g(t + 1)` is
// `sr2AXIT_EE1gplfp_Li1EE`) and write a template template parameter as
// `T_` (`TP<T>::g` is `srT_IT0_E1g`). `AX<T>::g(t)` is equivalent to the
// `N::X<T>::g(t)` before it and takes its spelling (`sr1N1XIT_EE1g`; see
// dependent-expr-first-spelling-two-units-run.cpp). Inputs/unresolved-qualifier-defs.cpp
// defines each specialization under clang's name with an asm label, so the
// program links only if kiln's references carry those names.
//@ std: c++20
//@ expect: run
//@ args: %{test_dir}/Inputs/unresolved-qualifier-defs.cpp
//@ stdout: 9 10 11 12 13 14
extern "C" int printf(const char *, ...);
namespace N {
template <class T> struct X {
  static int g(T);
  struct Y { static int g(T); };
};
}
struct A {
  template <class U> struct Z { static int g(A); };
};
template <class T> using AX = N::X<T>;
template <class T> auto f9(T t) -> decltype(N::X<T>::Y::g(t));
template <class T> auto f10(T t) -> decltype(N::X<T>::g(t));
template <class T> auto f11(T t) -> decltype(T::template Z<int>::g(t));
template <class T> auto f12(T t) -> decltype(AX<T>::g(t));
template <template <class> class TP, class T> auto f13(T t) -> decltype(TP<T>::g(t));
template <class T> auto f14(T t) -> decltype(AX<T>::g(t + 1));
int main() {
  printf("%d %d %d %d %d %d\n", f9(1), f10(1), f11(A()), f12(1), f13<N::X>(1), f14(1));
}
