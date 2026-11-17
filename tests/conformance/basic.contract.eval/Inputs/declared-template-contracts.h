// The declarations shared by run-client-check-declared-template*.cpp and
// their definitions in declared-template-contracts-defs.cpp: function
// templates and members whose specializations the tests call without
// a definition in the calling translation unit. The predicates name
// parameters of non-dependent type (GCC 16.1 fails to build the
// caller-side check of a type-dependent one).
#include <cstddef>
#include <utility>
struct S {
  template <class T> int m(const int x, T t) const pre(x > 0) post(r: r > x);
};
template <class T> int f(const int x, T t) pre(x > 0) post(r: r > x);
template <> int f<long>(const int x, long t) pre(x > 5);
template <class T> struct C {
  int m(const int x, T t) pre(x > 0) post(r: r > x);
};
template <class T> int e(int x, T t) pre(x > 0) { return 1; }
extern template int e<int>(int, int);
struct P {
  int v;
  template <std::size_t I> int get() const pre(v > 0);
};
template <> struct std::tuple_size<P> : std::integral_constant<std::size_t, 2> {};
template <std::size_t I> struct std::tuple_element<I, P> { using type = int; };
