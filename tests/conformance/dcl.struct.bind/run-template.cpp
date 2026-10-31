//@ expect: run
//@ stdout: 3 7 12 9 30
// [dcl.struct.bind] in templates ([temp.inst]): a structured binding
// declaration in a function template is instantiated with the template, so
// the kind of decomposition (array, data members, tuple-like) is chosen per
// specialization.
extern "C" int printf(const char *, ...);
namespace std {
template <class T> struct tuple_size;
template <unsigned long I, class T> struct tuple_element;
}
struct P { int x, y; };
struct Q {
  int a, b;
  template <unsigned long I> int get() const { return I == 0 ? a : b * 2; }
};
template <> struct std::tuple_size<Q> { static constexpr unsigned long value = 2; };
template <unsigned long I> struct std::tuple_element<I, Q> { using type = int; };
template <class T> int sum(const T &t) {
  auto [u, v] = t;
  return u + v;
}
template <class T> int loop(T (&ts)[2]) {
  int s = 0;
  for (const auto &[u, v] : ts) s += u * v;
  return s;
}
template <class T> int lam(T t) {
  auto &[u, v] = t;
  return [&] { return u * 10 + v; }();
}
int main() {
  int arr[2] = {3, 4};
  P ps[2] = {{1, 1}, {2, 4}};
  printf("%d %d %d %d %d\n", sum(P{1, 2}), sum(arr), sum(Q{4, 4}), loop(ps), lam(P{2, 10}));
}
