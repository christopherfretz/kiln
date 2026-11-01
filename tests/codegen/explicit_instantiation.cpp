// [temp.explicit]: an explicit instantiation definition gives every
// entity it defines `weak_odr` linkage in its own comdat (clang
// `GetGVALinkageForFunction` -> GVA_StrongODR), inline or not. Naming
// a class defines all of its members that are not templates, used or
// not, including members of member classes and out-of-line static
// data members ([temp.explicit]/10); the vtable and RTTI are weak_odr.
// Base-object structors of a class without virtual bases share a C5/D5
// comdat and the complete-object symbols are weak_odr aliases. Static
// locals stay linkonce_odr.
template <class T> T twice(T v) { return v + v; }
template int twice<int>(int);
template <class T> inline T thrice(T v) { return v * 3; }
template long thrice(long);
template <class T> constexpr T sq(T v) { return v * v; }
template short sq<short>(short);
template <class T> int counter() { static int n = 0; return ++n; }
template int counter<char>();

template <class T> struct X {
  X() {}
  X(T t) : v(t) {}
  ~X() {}
  T get() const { return v; }
  void set(T t);
  static T s;
  static inline T si = 7;
  struct In {
    int q() { return 3; }
    static int z;
  };
  T v{};
};
template <class T> void X<T>::set(T t) { v = t; }
template <class T> T X<T>::s = 9;
template <class T> int X<T>::In::z = 4;
template struct X<long>;

template <class T> struct P {
  virtual ~P() {}
  virtual T f() { return T(); }
};
template struct P<int>;

template <class T> struct Y {
  void m();
  void unused();
  Y();
  ~Y();
};
template <class T> void Y<T>::m() {}
template <class T> void Y<T>::unused() {}
template <class T> Y<T>::Y() {}
template <class T> Y<T>::~Y() {}
template void Y<int>::m();
template Y<int>::Y();
template Y<int>::~Y();
