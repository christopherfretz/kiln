// Mangling differential: C++26 pack indexing (P2662,
// [expr.prim.pack.index], [dcl.type.pack.index]) in dependent signatures
// ([temp.over.link]/5; spec/abi.md "Mangling", WP-1029). The Itanium ABI
// has no production for either; clang 19 and 22 agree:
// - a `pack...[i]` expression writes nothing: `decltype(t...[0])` is
//   `DTE`, an operand vanishes (`DTplLi1EE` for `t...[0] + 1`), and a
//   template argument has no `X…E` wrapper;
// - a `T...[i]` type is its pattern (`T_`) while dependent, the index
//   unencoded, and is a substitution candidate of its own after the
//   pattern: `f(T...[0], T...[0], T...)` is `T_S1_DpS0_`.
// (`S<I...[0]>` for an NTTP pack is clang-22-only; see
// tests/conformance/dcl.type.pack.index.)
// A member template of a class template specialization keeps the
// pack-index expression once the class's pack is substituted, so it
// still writes nothing: `Y<1, 2>::g` is `…DTplfp_ET_`, not the selected
// element `Li2E` (WP-1060).
template <int N> struct S {};
template <class T> struct W {};
template <class... T> auto pr(T... t) -> decltype(t...[0]);
template <class... T> auto pr1(T... t) -> decltype(t...[1]);
template <class... T> auto prp(T... t) -> decltype((t...[0]));
template <class... T> auto prs(T... t) -> decltype(t...[0] + 1);
template <class... T> auto pt(T...[0] x, T... t) -> void;
template <class... T> auto ptr(T... t) -> T...[1];
template <class... T> auto ptw(T... t) -> W<T...[0]>;
template <int I, class... T> auto pr2(T... t) -> decltype(t...[I]);
template <int I, class... T> auto pt2(T... t) -> T...[I];
template <int... I> auto pa3() -> decltype(I...[0]);
template <class... T> auto psz(T... t) -> S<sizeof(t...[0])>;
template <class... T> void sf(T...[0] a, T...[0] b, T... t);
template <class... T> void sg(T...[0], T...[1], T...[0] *);
template <class... T> void sh(const T...[0] &, T...[0] *);
template <int I, class... T> void sk(T...[I], T...[I + 1]);
template <int N, class U> int q(U);
template <int... I> struct Y {
  template <class U> static auto g(U u) -> decltype(u + I...[1]);
  template <class U> static auto g2(U u) -> decltype(I...[1]);
  template <class U> static auto g3(U u) -> decltype(u + I...[0] + I...[1]);
  template <class U, int J = I...[0]> static auto g4(U u) -> decltype(u + J);
  template <class U> static auto h2(U u) -> decltype(q<I...[1]>(u));
  template <class U> static auto h3(U u) -> decltype(u[I...[1]]);
  template <class U> static auto h4(U u) -> decltype((u, I...[1]));
};
template <class... T> struct Z {
  template <class U> static auto f(U u, T... t) -> decltype(u + t...[1]);
  template <class U>
  static auto c(U u, T... t) -> decltype(u(t...[0], t...[1]));
};
int k(int, long);

void use() {
  pr(1, 2.0);
  pr1(1, 2.0);
  prp(1, 2.0);
  prs(1, 2.0);
  pt<int, int>(1, 1, 2);
  ptr(1, 2.0);
  ptw(1);
  pr2<1>(1, 2.0);
  pt2<1>(1, 2.0);
  pa3<3, 4>();
  psz(1);
  int i = 0;
  long l = 0;
  sf<int, long>(1, 2, 3, 4L);
  sg<int, long>(1, 2L, &i);
  sh<int>(i, &i);
  sk<0, int, long>(1, 2L);
  (void)l;
  int a[4] = {};
  Y<1, 2>::g(1);
  Y<1, 2>::g2(1);
  Y<1, 2>::g3(1);
  Y<1, 2>::g4(1);
  Y<1, 2>::h2(1);
  Y<1, 2>::h3(a);
  Y<1, 2>::h4(1);
  Z<int, long>::f(1, 2, 3L);
  Z<int, long>::c(k, 2, 3L);
}
