// [dcl.type.pack.index], [expr.prim.pack.index] (C++26, P2662) in
// dependent signatures ([temp.over.link]/5; spec/abi.md "Mangling",
// WP-1029): the Itanium ABI has no production, and clang writes a
// `pack...[i]` expression as nothing and a `T...[i]` type as its
// pattern while dependent (`DTplfp_tliEE` for `u + T...[0]{}` with
// `T...` = `int, long`; the substituted type is the selected element).
// This unit only declares the templates; Inputs/mangling-defs.cpp
// defines each specialization under clang's name with an asm label, so
// the program links only if kiln's references carry those names.
//@ expect: run
//@ args: %{test_dir}/Inputs/mangling-defs.cpp
//@ stdout: 1 2 3 4 5
extern "C" int printf(const char *, ...);
template <class... T> struct X {
  template <class U> static auto f(U u) -> decltype(u + T...[0]{});
  template <class U> static auto g(U u) -> T...[1];
  template <class U> static auto h(U u, T... t) -> decltype(t...[0]);
};
template <class... T> auto pr(T... t) -> decltype(t...[0]);
template <class... T> auto pt(T...[0] x, T... t) -> int;
int main() {
  printf("%d %d %d %d %d\n", X<int, long>::f(1), int(X<int, long>::g(1)),
         X<int, long>::h(1, 2, 3L), pr(4, 2.0), pt<int, int>(1, 1, 2));
}
