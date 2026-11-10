// Mangling differential: `sizeof`, `alignof`, GNU `__alignof__` and
// `__datasizeof` operands in dependent signatures ([expr.sizeof],
// [expr.alignof], [temp.over.link]/5; spec/abi.md "Mangling", WP-1226).
// clang 19 and 22 agree:
// - a dependent operand is `st <type>` / `sz <expr>` for `sizeof`,
//   `at <type>` / `az <expr>` for `alignof`;
// - GNU `__alignof__` (the preferred alignment) is the vendor extended
//   expression `u11__alignof__ <type> E`, or `u11__alignof__
//   <template-arg> E` for an expression (`XfL0p_E`, wrapped as a
//   template argument);
// - a non-dependent operand is its value's literal (`Lm8E`), an
//   `alignas` on a named variable or member included, `__datasizeof` a
//   non-POD class's size without tail padding (`Lm9E` for `E`);
// - `sizeof...` is `sZ`.
template <unsigned long N> struct A {};
struct S {
  int a;
  char b;
};
struct D {
  long l;
  char c;
};
struct E {
  E();
  long l;
  char c;
};
template <class T> struct W {
  using type = T;
};
alignas(16) int g;
struct M {
  alignas(32) char c;
};
template <class T> void s1(A<sizeof(T)>) {}
template <class T> void s2(T t, A<sizeof t>) {}
template <class... T> void s3(A<sizeof...(T)>) {}
template <class T> void a1(A<alignof(T)>) {}
template <class T> void a2(T t, A<alignof(t)>) {}
template <class T> void p1(A<__alignof__(T)>) {}
template <class T> void p2(T t, A<__alignof__(t)>) {}
template <class T> void p3(T t, A<__alignof__(t + 1)>) {}
template <class T> void p4(A<__alignof__(typename W<T>::type)>) {}
template <class T> void p5(A<__alignof__(T) + sizeof(int)>) {}
template <class T> void p6(A<__alignof__(T) + __alignof__(double)>) {}
template <class T> void p7(A<__alignof__(T[sizeof(T)])>) {}
template <class T> auto d1(T t) -> decltype(t + sizeof(S)) { return 0; }
template <class T> auto d2(T t) -> decltype(t + alignof(S)) { return 0; }
template <class T> auto d3(T t) -> decltype(t + __alignof__(S)) { return 0; }
template <class T> auto d4(T t) -> decltype(t + __alignof__(t)) { return 0; }
template <class T> auto e1(T t) -> decltype(t + alignof(g)) { return 0; }
template <class T> auto e2(T t) -> decltype(t + __alignof__(g)) { return 0; }
template <class T> auto e3(T t) -> decltype(t + sizeof(g)) { return 0; }
template <class T> auto e4(T t) -> decltype(t + __datasizeof(D)) { return 0; }
template <class T> auto e5(T t) -> decltype(t + alignof(M::c)) { return 0; }
template <class T> auto e6(T t) -> decltype(t + __alignof__(int &)) { return 0; }
template <class T> auto e7(T t) -> decltype(t + __datasizeof(int)) { return 0; }
template <class T> auto e8(T t) -> decltype(t + __datasizeof(E)) { return 0; }
void use() {
  s1<int>({}); s2<int>(1, {}); s3<int, char>({});
  a1<int>({}); a2<int>(1, {});
  p1<int>({}); p2<int>(1, {}); p3<int>(1, {}); p4<int>({});
  p5<int>({}); p6<int>({}); p7<int>({});
  d1(1); d2(1); d3(1); d4(1);
  e1(1); e2(1); e3(1); e4(1); e5(1); e6(1); e7(1); e8(1);
}
