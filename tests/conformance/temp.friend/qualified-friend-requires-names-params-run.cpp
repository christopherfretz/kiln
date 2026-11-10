// [temp.friend]/1, [temp.over.link]/5, [temp.over.link]/7: a friend
// `template <class U> friend int A<T>::f(U u, W<T> &) requires (sizeof(u) >
// 1);` in class template W, whose trailing requires-clause names the
// function's parameters, redeclares the member template of A<int> with the
// equivalent clause when W<int> is instantiated: the parameters compare by
// position and substituted type (WP-1146; kiln reported an internal error
// substituting `u` before the friend's parameters were declared). So for a
// requires-expression naming one (g), an abbreviated member (p) and an
// overload set differing only by such a clause (o, p): the friend names the
// constrained member only, and the other one cannot read W's private member.
// clang 19 and GCC 16 print 1 2 3 0 4 0.
//@ skip(clang): clang 22 does not match these friends ("'s' is a private member of 'W<int>'"); clang 19 does
//@ expect: run
//@ stdout: 1 2 3 0 4 0
#include <cstdio>
template <class T> class W;
template <class T> struct A {
  template <class U> static int f(U u, W<T> &) requires (sizeof(u) > 1);
  template <class U> static int g(U u, W<T> &w) requires requires { u + 1; };
  template <class U> static int o(U u, W<T> &) requires (sizeof(u) > 1);
  template <class U> static int o(U u, W<T> &);
  static int p(auto u, W<T> &) requires (sizeof(u) > 1);
  static int p(auto u, W<T> &);
};
template <class T> class W {
  int s[4] = {1, 2, 3, 4};
  template <class U> friend int A<T>::f(U v, W<T> &) requires (sizeof(v) > 1);
  template <class U> friend int A<T>::g(U u, W<T> &w) requires requires { u + 1; };
  template <class U> friend int A<T>::o(U u, W<T> &) requires (sizeof(u) > 1);
  friend int A<T>::p(auto u, W<T> &) requires (sizeof(u) > 1);
};
template <class T> template <class U> int A<T>::f(U u, W<T> &w) requires (sizeof(u) > 1) { return w.s[0]; }
template <class T> template <class U> int A<T>::g(U u, W<T> &w) requires requires { u + 1; } { return w.s[1]; }
template <class T> template <class U> int A<T>::o(U u, W<T> &w) requires (sizeof(u) > 1) { return w.s[2]; }
template <class T> template <class U> int A<T>::o(U, W<T> &w) { return requires { w.s; } ? 100 : 0; }
template <class T> int A<T>::p(auto u, W<T> &w) requires (sizeof(u) > 1) { return w.s[3]; }
template <class T> int A<T>::p(auto, W<T> &w) { return requires { w.s; } ? 100 : 0; }
int main() {
  W<int> w;
  std::printf("%d %d %d %d %d %d\n", A<int>::f(1, w), A<int>::g(1, w), A<int>::o(1, w),
              A<int>::o('c', w), A<int>::p(1, w), A<int>::p('c', w));
}
