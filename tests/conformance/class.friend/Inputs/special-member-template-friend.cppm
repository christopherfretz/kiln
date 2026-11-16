// Module `smtf`: class templates befriending a constructor template and a
// conversion function template of a dependent class, and a constructor of
// every specialization of a class template
// (special-member-template-friend-module-run.cpp).
export module smtf;
extern "C" int printf(const char *, ...);
export template <class T> struct Z {
  template <class U> Z(U);
  template <class U> operator U*();
};
export template <class T> struct X { X(T); };
export template <class T> class Y {
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
