// [temp.friend]/5, [temp.over.link]/6-7, [basic.scope.scope]/4: in an
// overload set whose members differ only by their constraints, a friend
// `template <class T> template <class U, class Y> friend int A<T>::o(U,
// const B<Y> &) requires C<U>;` names the member with equivalent
// constraints only (WP-1130): the constrained `o` reads B's private member
// and the unconstrained one cannot (its requires-expression is false). The
// same for a member template overloaded on a requires-clause naming the
// class's parameter (p) and an abbreviated `C auto` overload (v). (A
// non-template member friend with a trailing requires-clause is ill-formed,
// [temp.friend]/9: non-template-member-friend-requires-not-definition.cpp,
// WP-1146.) The friends
// are in a class template, so their heads are compared with the
// instantiation's arguments; equivalent type-constraints, requires-clauses
// and an abbreviated head are befriended (f, g, h, i). clang turns off
// access control for B instead (-Wunsupported-friend).
//@ skip(clang): clang turns off access control for the befriending class
//@ expect: run
//@ strict-diags
//@ stdout: 1 1 1 1 1 200 1 400 1 800
//@ diag: 44:63 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 46:20 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 48:20 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 49:58 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 51:20 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 52:58 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 53:58 warning "for friend class declaration is not supported; turning off access control for 'B'"
#include <cstdio>
template <class T> concept C = sizeof(T) > 1;
template <class T, class V> concept D = sizeof(T) >= sizeof(V);
template <class X> class B;
template <class T> struct A {
  template <C U, class X> static int f(U, const B<X> &);
  template <class U, class X> requires D<U, T> static int g(U, const B<X> &);
  template <class U, class X> static int h(U, const B<X> &) requires C<U>;
  template <class X> static int i(C auto, const B<X> &);
  template <class U, class X> static int o(U, const B<X> &);
  template <class U, class X> static int o(U, const B<X> &) requires C<U>;
  template <class X> static int p(const B<X> &) requires C<T>;
  template <class X> static int p(const B<X> &) requires (!C<T>);
  template <class X> static int v(auto, const B<X> &);
  template <class X> static int v(C auto, const B<X> &);
};
template <class X> class B {
  template <class T> template <C U, class Y> friend int A<T>::f(U, const B<Y> &);
  template <class T> template <class U, class Y> requires D<U, T>
  friend int A<T>::g(U, const B<Y> &);
  template <class T> template <class U, class Y>
  friend int A<T>::h(U, const B<Y> &) requires C<U>;
  template <class T> template <class Y> friend int A<T>::i(C auto, const B<Y> &);
  template <class T> template <class U, class Y>
  friend int A<T>::o(U, const B<Y> &) requires C<U>;
  template <class T> template <class Y> friend int A<T>::p(const B<Y> &) requires C<T>;
  template <class T> template <class Y> friend int A<T>::v(C auto, const B<Y> &);
  int n = 1;
};
template <class T> template <C U, class X> int A<T>::f(U, const B<X> &b) { return b.n; }
template <class T> template <class U, class X> requires D<U, T>
int A<T>::g(U, const B<X> &b) { return b.n; }
template <class T> template <class U, class X>
int A<T>::h(U, const B<X> &b) requires C<U> { return b.n; }
template <class T> template <class X> int A<T>::i(C auto, const B<X> &b) { return b.n; }
template <class T> template <class U, class X> int A<T>::o(U, const B<X> &b) {
  return requires { b.n; } ? 100 : 200;
}
template <class T> template <class U, class X>
int A<T>::o(U, const B<X> &b) requires C<U> { return b.n; }
template <class T> template <class X> int A<T>::p(const B<X> &b) requires C<T> { return b.n; }
template <class T> template <class X> int A<T>::p(const B<X> &b) requires (!C<T>) {
  return requires { b.n; } ? 300 : 400;
}
template <class T> template <class X> int A<T>::v(auto, const B<X> &b) {
  return requires { b.n; } ? 700 : 800;
}
template <class T> template <class X> int A<T>::v(C auto, const B<X> &b) { return b.n; }
int main() {
  B<char> b;
  std::printf("%d %d %d %d %d %d %d %d %d %d\n", A<int>::f(1, b), A<int>::g(1, b),
              A<int>::h(1, b), A<int>::i(1, b), A<int>::o(1, b), A<int>::o('c', b),
              A<int>::p(b), A<char>::p(b), A<int>::v(1, b),
              A<int>::v('c', b));
}
