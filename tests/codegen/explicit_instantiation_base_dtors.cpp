// [class.dtor]/13: an explicitly instantiated class template
// specialization destroys its members, then its direct bases in reverse
// declaration order; the base destructors (implicitly instantiated, or
// explicitly instantiated before or after the derived class) are emitted
// in clang's order, with weak_odr D5 comdats for the explicit ones and
// thunks in their own comdats.
extern "C" int puts(const char *);
template <class T, int N> struct A { ~A() { puts("A"); } };
template <class T> struct X : A<T, 1>, A<T, 2>, A<T, 3> { T t; ~X() {} };
template struct X<int>;

template <class T> struct M { ~M() {} };
template <class T> struct Y : A<T, 4>, A<T, 5> { M<T> m; A<T, 6> a; };
template struct Y<int>;
void use_y() { Y<int> y; }

template <class T> struct P { ~P() {} };
template <class T> struct Q { ~Q() {} };
template <class T> struct R { ~R() {} };
template <class T> struct Z : P<T>, Q<T>, R<T> { ~Z() {} void f(); };
template <class T> void Z<T>::f() { R<T> r; P<T> p; Q<T> q; }
template struct Z<int>;

template struct P<long>;
template <class T> struct W : P<long>, Q<long> { ~W() {} };
template struct W<int>;
template struct Q<long>;

template <class T> struct V1 { virtual ~V1() {} };
template <class T> struct V2 { virtual ~V2() {} };
template <class T> struct VD : V1<T>, V2<T> { ~VD() override {} };
template struct VD<int>;

template <class T> struct VB : virtual A<T, 7>, A<T, 8>, virtual A<T, 9> {
  virtual ~VB() {}
};
template struct VB<int>;
