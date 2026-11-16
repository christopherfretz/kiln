// [temp.friend]/5: `template<class U> friend X<U>::X(U);` and `template<class
// U> friend X<U>::~X();` befriend that constructor and the destructor of
// every specialization of `X`, not its other constructors (sibling of
// friend-constructor-template-of-class-template-run.cpp).
//@ skip(clang): clang 22 rejects a friend constructor of every specialization of a class template
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:49 error "'x' is a private member of 'Y<char>'"
//@ diag: 21:25 note "in instantiation of member function 'X<int>::X' requested here"
//@ diag: 15:14 note "implicitly declared private here"
template <class T> struct X { X(T); X(T*); ~X(); };
template <class T> class Y {
  template <class U> friend X<U>::X(U);
  template <class U> friend X<U>::~X();
  static int x;
};
template <class T> int Y<T>::x = 3;
template <class T> X<T>::X(T) { (void)Y<char>::x; }
template <class T> X<T>::X(T*) { (void)Y<char>::x; }
template <class T> X<T>::~X() { (void)Y<char>::x; }
void g() { X<int> a(1), b((int*)0); }
