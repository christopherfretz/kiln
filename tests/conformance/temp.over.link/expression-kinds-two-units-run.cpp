// Function template specializations whose signatures hold the expression
// kinds of WP-1736 ([temp.over.link]/5, Itanium C++ ABI 5.1.6
// <expression>; spec/abi.md §8.15): declared here and defined in
// Inputs/expression-kinds-defs.cpp under the names clang 19 and 22 give
// them (asm labels), so this links only if kiln's names are clang's:
// `sZ fp_` (`sizeof...` of a function parameter pack, which kiln
// rejected with "cannot yet mangle expression type SizeOfPackExpr"),
// `sP` (a partially substituted pack), a `decltype` qualifier
// (`sr1S1g`, which kiln rejected with "cannot yet mangle expression type
// UnresolvedLookupExpr"), a rewritten `!=`, designated initializers,
// `v110_SUBSTPACK` and a non-dependent `decltype(t + 1)` (`i`).
//@ expect: run
//@ args: %{test_dir}/Inputs/expression-kinds-defs.cpp
//@ stdout: 8
extern "C" int printf(const char *, ...);
template <unsigned long N> struct A {
  typedef int type;
};
template <class... T> auto fp_pack(T... a) -> typename A<sizeof...(a)>::type;
template <class... T> using Al = A<sizeof...(T)>;
template <class... U> int partial(Al<int, U...>);
struct S {
  template <class T> static T g(T t) { return t; }
};
S s;
template <class T> auto dq(T t) -> decltype(decltype(s)::g(t));
struct X {
  bool operator==(const X &) const { return true; }
};
template <class T> auto rw(T t) -> decltype(t, X{} != X{});
struct P {
  int a, b;
};
int k(P p);
template <class T> auto desig(T t) -> decltype(k({.a = t, .b = 2}));
template <class Ta, class Tb> int k2(Ta, Tb);
template <class... Z> struct L {};
template <class... T> struct Q {
  template <class... U> static auto h(T... t, U... u) -> L<decltype(k2(t, u))...>;
};
struct N {
  template <class U> static auto nd(int t, U) -> decltype(t + 1);
};
int count();
int main() {
  fp_pack(1, 2);
  partial<char>(A<2>{});
  dq(1);
  rw(1);
  desig(1);
  Q<int, long>::h(1, 2L, 3, 4);
  N::nd(1, 2);
  printf("%d\n", count() + 1);
}
