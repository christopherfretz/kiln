// [expr.prim.pack.index] (C++26, P2662) as a template argument in a
// dependent signature ([temp.over.link]/5; spec/abi.md "Mangling",
// WP-1029): clang 22 writes `S<I...[0]>` with no `X…E` wrapper and the
// pack-index expression as nothing (`1SIE`); a substituted one is the
// selected value (`1SILi2EE`). clang 19 rejects an NTTP pack index as a
// template argument. Inputs/template-arg-mangling-defs.cpp defines each
// specialization under clang 22's name with an asm label.
//@ requires: clang>=22
//@ expect: run
//@ args: %{test_dir}/Inputs/template-arg-mangling-defs.cpp
//@ stdout: 1 2
extern "C" int printf(const char *, ...);
template <int N> struct S {};
template <int... I> auto pa(S<I...[0]>) -> int;
template <int... I> struct Y {
  template <class U> static auto f(U u) -> S<I...[1]>;
};
int yf();
int main() {
  Y<1, 2>::f(0);
  printf("%d %d\n", pa<3, 4>(S<3>{}), yf());
}
