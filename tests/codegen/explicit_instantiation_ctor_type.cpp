// [temp.explicit]/10, /11: an explicit instantiation definition of a
// class template instantiates its members (weak_odr, in comdats), the
// static data member's dynamic initializer included; `extern template`
// suppresses the implicit instantiation of the members it covers. The
// constructor creates the class's IR type in its prologue (clang
// `FieldConstructionScope`), so `X<long>` is named before `X<int>`.
int side();
template <class T> struct X {
  static int s;
  static int d;
  int f() { return 1; }
  static int sf() { return 2; }
  X() {}
  ~X() {}
};
template <class T> int X<T>::s = 3;
template <class T> int X<T>::d = side();
template struct X<long>;
template class X<char>;
extern template struct X<int>;
int use() { X<int> a; return a.f() + X<int>::d; }
