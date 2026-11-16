// [temp.expl.spec]: a member's explicit specialization matches when the
// class's arguments satisfy its trailing requires-clause; a constrained
// variable template is specialized for satisfying arguments.
//@ std: c++20
template<class T> concept Cls = __is_class(T);
struct E {};
template<class T> struct S {
  void f() requires Cls<T>;
  void g() requires (sizeof(T) > 1);
  void h();
};
template<> void S<E>::f() {}
template<> void S<int>::g() {}
template<> void S<int>::h() {}
template<class T, class U> concept Same = __is_same(T, U);
template<Same<int> T> int v = 0;
template<> int v<int> = 1;
int use() { return v<int>; }
