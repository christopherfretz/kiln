//@ expect: run
//@ requires: clang>=22
//@ oracle: none
//@ stdout: 1 2 65 | 3
//@ stdout: 4 5 | 2
//@ stdout: sum 21
// [dcl.struct.bind]/1 (C++26 P1061) at run time: a structured binding pack
// over a tuple-like class, an array and a class with data members, expanded
// in comma and plus folds. clang 19 (the default oracle) does not parse
// binding packs, so the output is checked by hand (`oracle: none`).
extern "C" int printf(const char *, ...);
namespace std {
  template <class T> struct tuple_size;
  template <unsigned long I, class T> struct tuple_element;
}
struct T3 { int a; int b; char c; };
template <unsigned long I> int get(const T3& t) { return I == 0 ? t.a : I == 1 ? t.b : t.c; }
template <> struct std::tuple_size<T3> { static constexpr unsigned long value = 3; };
template <unsigned long I> struct std::tuple_element<I, T3> { using type = int; };
struct M { int a, b, c, d, e, f; };
template <class T> int show(const T& t) {
  auto&& [...xs] = t;
  int n = 0;
  ((printf(n++ ? " %d" : "%d", int(xs))), ...);
  printf(" | %d\n", int(sizeof...(xs)));
  return n;
}
template <class T> int sum(const T& t) {
  const auto& [first, ...rest] = t;
  return (first + ... + rest);
}
int main() {
  int arr[2] = {4, 5};
  int n = show(T3{1, 2, 'A'}) + show(arr);
  printf("sum %d\n", sum(M{1, 2, 3, 4, 5, 6}));
  return n == 5 ? 0 : 1;
}
