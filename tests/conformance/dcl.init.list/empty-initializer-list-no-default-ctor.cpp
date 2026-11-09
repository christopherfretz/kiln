// [dcl.init.list]/3.5-3.6: an empty list for a std::initializer_list<E>
// specialization without a default constructor constructs an empty
// initializer_list (3.6); clang accepts it. kiln used to value-initialize,
// report "no matching constructor for initialization of
// 'std::initializer_list<int>'" for the member 'a', and then overflow its
// stack on the aggregate C (fixed by WP-962). Reduced from clang's CXX/drs/cwg10xx.cpp (CWG1070,
// found by the clang negative-test run, WP-912).
//@ std: c++11
//@ strict-diags
//@ expect: compile-ok
namespace std {
typedef decltype(sizeof 0) size_t;
template <typename T> struct initializer_list {
  const T *p;
  size_t n;
  initializer_list(const T *p, size_t n);
};
} // namespace std
struct A {
  A(std::initializer_list<int>);
};
struct C {
  std::initializer_list<int> a;
  A b;
};
C c = {};
