//@ expect: ast
//@ ast-filter: test
//@ requires: clang>=22
// [dcl.struct.bind]/4 (C++26 /7): a tuple-like type. std::tuple_size<E> is
// complete, so each binding is a variable of type "reference to
// std::tuple_element<i, E>::type" initialized from `e.get<i>()` (a member
// get template, found by class member lookup). The holding variable is an
// xvalue in the initializer when E is not an lvalue reference.
namespace std {
template <class T> struct tuple_size;
template <unsigned long I, class T> struct tuple_element;
}
struct Q {
  int a;
  float b;
  template <unsigned long I> auto get() const {
    if constexpr (I == 0) return a; else return b;
  }
};
template <> struct std::tuple_size<Q> { static constexpr unsigned long value = 2; };
template <> struct std::tuple_element<0, Q> { using type = int; };
template <> struct std::tuple_element<1, Q> { using type = float; };
int test_value() {
  auto [m, n] = Q{1, 2.0f};
  return m + int(n);
}
int test_ref(Q &q) {
  auto &[m, n] = q;
  return m;
}
