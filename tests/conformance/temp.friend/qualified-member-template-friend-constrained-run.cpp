// [temp.friend]/1, [temp.over.link]/6-7, [basic.scope.scope]/4: a friend
// member function template of a class template whose qualifier names the
// enclosing class's parameter (`template <C U> friend int A<T>::f(U, W<T>
// &);` in W<T>) is matched when W is instantiated (WP-1110), against the
// member template of A<int> with equivalent constraints (WP-1145): a
// type-constraint (f), a trailing requires-clause (g), a head requires-clause
// (h), an abbreviated `auto` (i) and `C auto` (j) parameter. Of overloads that
// differ only by their constraints, the friend names the one with equivalent
// constraints (o, p): the other one cannot read W's private member. clang and
// g++ print 1 2 3 4 5 6 0 7 0.
//@ expect: run
//@ stdout: 1 2 3 4 5 6 0 7 0
#include <cstdio>
template <class T> concept C = sizeof(T) > 1;
template <class T> class W;
template <class T> struct A {
  template <C U> static int f(U, W<T> &);
  template <class U> static int g(U, W<T> &) requires C<U>;
  template <class U> requires C<U> static int h(U, W<T> &);
  static int i(auto, W<T> &);
  static int j(C auto, W<T> &);
  template <class U> static int o(U, W<T> &) requires C<U>;
  template <class U> static int o(U, W<T> &);
  static int p(C auto, W<T> &);
  static int p(auto, W<T> &);
};
template <class T> class W {
  int s[8] = {1, 2, 3, 4, 5, 6, 7, 8};
  template <C U> friend int A<T>::f(U, W<T> &);
  template <class U> friend int A<T>::g(U, W<T> &) requires C<U>;
  template <class U> requires C<U> friend int A<T>::h(U, W<T> &);
  friend int A<T>::i(auto, W<T> &);
  friend int A<T>::j(C auto, W<T> &);
  template <class U> friend int A<T>::o(U, W<T> &) requires C<U>;
  friend int A<T>::p(C auto, W<T> &);
};
template <class T> template <C U> int A<T>::f(U, W<T> &w) { return w.s[0]; }
template <class T> template <class U> int A<T>::g(U, W<T> &w) requires C<U> { return w.s[1]; }
template <class T> template <class U> requires C<U> int A<T>::h(U, W<T> &w) { return w.s[2]; }
template <class T> int A<T>::i(auto, W<T> &w) { return w.s[3]; }
template <class T> int A<T>::j(C auto, W<T> &w) { return w.s[4]; }
template <class T> template <class U> int A<T>::o(U, W<T> &w) requires C<U> { return w.s[5]; }
template <class T> template <class U> int A<T>::o(U, W<T> &w) {
  if constexpr (requires { w.s; }) return w.s[0];
  else return 0;
}
template <class T> int A<T>::p(C auto, W<T> &w) { return w.s[6]; }
template <class T> int A<T>::p(auto, W<T> &w) {
  if constexpr (requires { w.s; }) return w.s[0];
  else return 0;
}
int main() {
  W<int> w;
  std::printf("%d %d %d %d %d %d %d %d %d\n", A<int>::f(1, w), A<int>::g(1, w), A<int>::h(1, w),
              A<int>::i(1, w), A<int>::j(1, w), A<int>::o(1, w), A<int>::o('c', w),
              A<int>::p(1, w), A<int>::p('c', w));
}
