// A throw-expression in a dependent signature ([temp.over.link]/5,
// [expr.throw]; Itanium `tw <expression>`; spec/abi.md "Mangling",
// WP-1029): `decltype(throw t)` is `DTtwfp_E` and a conditional with a
// throw arm (an `int &` lvalue here) `DTqufp_twfp_fp_E`; `decltype(throw)` is not dependent and is
// just `v`. The mangle differential runs -fno-exceptions, so this
// production is pinned here: Inputs/throw-expression-defs.cpp defines
// each specialization under clang 19's name with an asm label.
//@ expect: run
//@ args: -Wno-unevaluated-expression %{test_dir}/Inputs/throw-expression-defs.cpp
//@ stdout: 7
extern "C" int printf(const char *, ...);
template <class T> auto s8(T t) -> decltype(throw t);
template <class T> auto s9(T t) -> decltype(throw);
template <class T> auto s10(T t) -> decltype(t ? throw t : t);
int count();
int main() {
  s8(1);
  s9(1);
  printf("%d\n", s10(5) + count());
}
