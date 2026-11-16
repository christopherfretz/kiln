// [temp.friend]/1, [class.friend]/4: `template<class U> friend
// Z<T>::Z(U);` and `template<class U> friend Z<T>::operator U*();` in a
// class template befriend, in each specialization `Y<A>`, every
// specialization of the constructor template and conversion function
// template of `Z<A>` (found running GCC's C++ testsuite). The sibling
// dependent-special-member-template-friend-denied.cpp keeps the other
// members private.
//@ skip(clang): clang 22 befriends no constructor template of a dependent class
//@ expect: run
//@ stdout: 3 3 3
extern "C" int printf(const char *, ...);
template <class T> struct Z {
  template <class U> Z(U);
  template <class U> operator U*();
};
template <class T> class Y {
  template <class U> friend Z<T>::Z(U);
  template <class U> friend Z<T>::operator U*();
  static int x;
};
template <class T> int Y<T>::x = 3;
template <class T> template <class U> Z<T>::Z(U) { printf("%d ", Y<T>::x); }
template <class T> template <class U> Z<T>::operator U*() {
  printf("%d", Y<T>::x);
  return 0;
}
int main() {
  Z<int> a(1), b(2L);
  (void)(char *)a;
  printf("\n");
}
