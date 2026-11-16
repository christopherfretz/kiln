// A class template befriending a constructor, the destructor and a
// conversion function of a dependent class (dependent-special-member-
// friend-pch-run.cpp, dependent-special-member-friend-module-run.cpp).
extern "C" int printf(const char *, ...);
template <class T> class Y;
template <class T> struct Z {
  Z(typename T::t);
  ~Z();
  operator int();
};
template <class T> class Y {
  friend Z<T>::Z(typename T::t);
  friend Z<T>::~Z();
  friend Z<T>::operator int();
  static int x;
};
template <class T> int Y<T>::x = 1;
template <class T> Z<T>::Z(typename T::t) { printf("%d ", Y<T>::x); }
template <class T> Z<T>::~Z() { printf("%d\n", Y<T>::x + 2); }
template <class T> Z<T>::operator int() { return Y<T>::x + 1; }
