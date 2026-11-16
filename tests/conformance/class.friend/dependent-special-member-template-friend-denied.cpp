// [temp.friend]/1: a friend constructor template or conversion function
// template of a dependent class befriends that member template of each
// specialization only, not the class's non-template constructor or
// conversion function (sibling of
// dependent-special-member-template-friend-run.cpp).
//@ skip(clang): clang 22 befriends no constructor template of a dependent class
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:49 error "'x' is a private member of 'Y<int>'"
//@ diag: 31:25 note "in instantiation of member function 'Z<int>::Z' requested here"
//@ diag: 24:14 note "implicitly declared private here"
//@ diag: 30:55 error "'x' is a private member of 'Y<int>'"
//@ diag: 31:66 note "in instantiation of member function 'Z<int>::operator int' requested here"
//@ diag: 24:14 note "implicitly declared private here"
template <class T> struct Z {
  template <class U> Z(U);
  Z(int *);
  template <class U> operator U*();
  operator int();
};
template <class T> class Y {
  template <class U> friend Z<T>::Z(U);
  template <class U> friend Z<T>::operator U*();
  static int x;
};
template <class T> int Y<T>::x = 3;
template <class T> template <class U> Z<T>::Z(U) { (void)Y<T>::x; }
template <class T> Z<T>::Z(int *) { (void)Y<T>::x; }
template <class T> template <class U> Z<T>::operator U*() { (void)Y<T>::x; return 0; }
template <class T> Z<T>::operator int() { (void)Y<T>::x; return 0; }
void g() { Z<int> a(1), c((int *)0); (void)(char *)a; (void)(int)a; }
