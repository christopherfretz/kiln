// [over.match.class.deduct]/1 (P2582R1): the guide inherited from a base's
// constructor keeps that constructor's access context, so a protected
// member type of the base's own base deduces. Found running GCC's C++
// testsuite.
//@ expect: compile-ok
//@ std: c++23
//@ skip(clang): neither clang 19 nor clang 22 implements P2582
template <class T> struct Base {
protected:
  using type = T;
};
template <class T> struct C1 : Base<T> {
  C1(T, typename Base<T>::type);
};
template <class T> struct D1 : C1<T> {
  using C1<T>::C1;
};
D1 d1(1, 1);
static_assert(__is_same(decltype(d1), D1<int>));
