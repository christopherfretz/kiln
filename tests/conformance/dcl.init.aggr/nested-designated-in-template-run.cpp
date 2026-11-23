// [dcl.init.aggr]/3.1, [temp.inst]: a designated initializer list nested
// many levels deep in a class template and in a function template, with
// dependent and non-dependent clauses, initializes every level when
// instantiated for several types (each written list keeps one written
// form since WP-2478; every level rebuilt the forms of the inner ones,
// (N + 1)(N + 2)/2 lists in all, before).
//@ expect: run
//@ stdout: 1 2 3 7 8 9 4
//@ stdout: 5 6 7 1 2 2 4
extern "C" int printf(const char*, ...);
template <class T> struct N0 { T x; int y; };
template <class T> struct N1 { N0<T> a; int b; };
template <class T> struct N2 { N1<T> a; T c; };
template <class T> struct N3 { N2<T> a; int d; };
template <class T> struct N4 { N3<T> a; };
template <class T> struct A {
  static inline N4<T> w = {.a = {.a = {.a = {.a = {.x = T(1), .y = 2}, .b = 3}, .c = T(7)},
                                 .d = 8}};
  static constexpr N4<T> k = {.a = {.a = {.a = {.a = {.x = T(9)}}}, .d = 4}};
};
template <class T> N4<T> make(T v) {
  return {.a = {.a = {.a = {.a = {.x = v, .y = 6}, .b = int(v) + 2}, .c = T(1)}, .d = 3}};
}
static_assert(A<int>::k.a.a.a.a.x == 9 && A<int>::k.a.a.a.a.y == 0 && A<int>::k.a.d == 4);
int main() {
  N4<int>& w = A<int>::w;
  N4<long> m = make(5L);
  printf("%d %d %d %d %d %d %d\n", w.a.a.a.a.x, w.a.a.a.a.y, w.a.a.a.b, w.a.a.c, w.a.d,
         int(A<short>::k.a.a.a.a.x), A<char>::k.a.d);
  printf("%d %d %d %d %d %d %d\n", int(m.a.a.a.a.x), m.a.a.a.a.y, m.a.a.a.b, int(m.a.a.c),
         int(A<double>::w.a.a.a.a.x) + 1, A<double>::w.a.a.a.a.y, m.a.d + 1);
  return 0;
}
