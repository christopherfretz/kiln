// Class templates befriending a constructor template and a conversion
// function template of a dependent class, and a constructor of every
// specialization of a class template
// (special-member-template-friend-pch-run.cpp,
// special-member-template-friend-module-run.cpp).
extern "C" int printf(const char *, ...);
template <class T> struct Z {
  template <class U> Z(U);
  template <class U> operator U*();
};
template <class T> struct X { X(T); };
template <class T> class Y {
  template <class U> friend Z<T>::Z(U);
  template <class U> friend Z<T>::operator U*();
  template <class U> friend X<U>::X(U);
  static int x;
};
template <class T> int Y<T>::x = 1;
template <class T> template <class U> Z<T>::Z(U) { printf("%d ", Y<T>::x); }
template <class T> template <class U> Z<T>::operator U*() {
  printf("%d ", Y<T>::x + 1);
  return 0;
}
template <class T> X<T>::X(T) { printf("%d\n", Y<char>::x + 2); }
