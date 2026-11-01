// [temp.explicit]/14: under an explicit instantiation declaration
// (`extern template`) no member definition is emitted (at -O0 not even
// inline ones), calls use the complete-object destructor symbol, and
// the vtable is an external declaration; static data members are
// external. Template patterns and instantiations have no key function
// (Itanium 5.2.6), so an implicit instantiation's vtable is linkonce_odr
// and no vtable is emitted for the pattern itself.
template <class T> struct X {
  virtual ~X() {}
  virtual T f() { return T(); }
  T g() { return f(); }
};
extern template class X<int>;
int use(X<int> &x) { X<int> y; return x.g() + y.f(); }
template <class T> struct Y { virtual void h(); };
template <class T> void Y<T>::h() {}
extern template struct Y<char>;
void use2() { Y<char> y; y.h(); }
void use3() { Y<short> y; y.h(); }
template <class T> struct S {
  static int x;
  static inline int y = 2;
  S() {}
  S(int) {}
  ~S() {}
  virtual void v() {}
};
template <class T> int S<T>::x = 1;
extern template struct S<int>;
struct D : S<int> { D() : S<int>(3) {} };
int *px = &S<int>::x;
int *py = &S<int>::y;
void ui() { D d; S<int> s; }
template <class T> T tf(T t) { return t; }
extern template int tf<int>(int);
int ut() { return tf(1); }
