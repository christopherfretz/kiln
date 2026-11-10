// [temp.friend]/5, [temp.over.link]/5, [temp.over.link]/7: a friend member
// template `template <class T> template <class U> friend int A<T>::f(U u,
// const B &) requires (sizeof(u) > 1);` whose trailing requires-clause names
// the function's parameters befriends the member of each A<T> with the
// equivalent clause: the parameters compare by position and type, not by
// name or declaration (WP-1146). So for a requires-expression naming one (g),
// an abbreviated member (h) and an overload set differing only by such a
// clause (o): the friend names the constrained `o` only, and the other `o`
// cannot read B's private member (its requires-expression is false). clang
// turns off access control for B instead (-Wunsupported-friend).
//@ skip(clang): clang turns off access control for the befriending class
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 3 4 0
//@ diag: 29:58 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 30:58 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 31:39 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 32:58 warning "for friend class declaration is not supported; turning off access control for 'B'"
#include <cstdio>
class B;
template <class T> struct A {
  template <class U> static int f(U u, const B &b) requires (sizeof(u) > 1);
  template <class U> static int g(U u, const B &b) requires requires { u + 1; };
  static int h(auto u, const B &b) requires (sizeof(u) > 1);
  template <class U> static int o(U u, const B &b) requires (sizeof(u) > 1);
  template <class U> static int o(U u, const B &b);
};
class B {
  template <class T> template <class U> friend int A<T>::f(U v, const B &) requires (sizeof(v) > 1);
  template <class T> template <class U> friend int A<T>::g(U u, const B &) requires requires { u + 1; };
  template <class T> friend int A<T>::h(auto u, const B &) requires (sizeof(u) > 1);
  template <class T> template <class U> friend int A<T>::o(U u, const B &) requires (sizeof(u) > 1);
  int n[4] = {1, 2, 3, 4};
};
template <class T> template <class U> int A<T>::f(U u, const B &b) requires (sizeof(u) > 1) { return b.n[0]; }
template <class T> template <class U> int A<T>::g(U u, const B &b) requires requires { u + 1; } { return b.n[1]; }
template <class T> int A<T>::h(auto u, const B &b) requires (sizeof(u) > 1) { return b.n[2]; }
template <class T> template <class U> int A<T>::o(U u, const B &b) requires (sizeof(u) > 1) { return b.n[3]; }
template <class T> template <class U> int A<T>::o(U, const B &b) {
  return requires { b.n; } ? 100 : 0;
}
int main() {
  B b;
  std::printf("%d %d %d %d %d\n", A<int>::f(1, b), A<int>::g(1, b), A<int>::h(1, b), A<int>::o(1, b),
              A<int>::o('c', b));
}
