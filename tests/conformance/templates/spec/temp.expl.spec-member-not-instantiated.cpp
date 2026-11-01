// [temp.expl.spec]/7: an explicitly specialized member of an implicit
// class template specialization is never instantiated from the pattern,
// even when the pattern's definition is ill-formed for those arguments
// (declared only, or defined by the specialization).
//@ expect: compile-ok
//@ strict-diags
template <class T> struct A {
  constexpr int f() const { return T::value; }
  int g() { return T::value; }
  static int h();
};
template <class T> int A<T>::h() { return T::value; }
template <> constexpr int A<int>::f() const { return 4; }
template <> int A<int>::g();
template <> int A<char>::h();
static_assert(A<int>().f() == 4);
int use() { return A<int>().g() + A<char>::h(); }
