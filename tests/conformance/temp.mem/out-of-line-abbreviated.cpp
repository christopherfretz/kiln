// [temp.mem]/1, [dcl.fct]/22: in an out-of-line definition of a member of
// a class template, `template<class T>` belongs to the class; a
// placeholder parameter (`C auto`, `auto...`) invents the member's own
// template parameter list rather than extending the class's (libc++'s
// out-of-line `C auto` members). With an explicit member head the
// invented parameters are appended to it.
//@ expect: run
//@ strict-diags
//@ exit: 0
template<class T> concept C = sizeof(T) >= 4;
template<class T> concept D = sizeof(T) >= 2;
template<class T> struct A {
  int f(C auto x);
  template<class U> int g(U u, C auto y);
  int h(auto z, auto... w);
  struct N { template<class V> struct M { int k(C auto q); }; };
};
template<class T> int A<T>::f(C auto x) { return sizeof(T) + sizeof(x); }
template<class T> template<class U> int A<T>::g(U u, C auto y) { return sizeof(u) + sizeof(y); }
template<class T> int A<T>::h(auto z, auto... w) { return sizeof(z) + sizeof...(w); }
template<class T> template<class V> int A<T>::N::M<V>::k(C auto q) { return sizeof(T) * 100 + sizeof(V) * 10 + sizeof(q); }
int main() {
  A<char> a;
  int r = a.f(1) * 1000 + a.g(1, 2L) * 100 + a.h('c', 1, 2) * 10;
  A<char>::N::M<short> m;
  return (r == 5000 + 1200 + 30 && m.k(1) == 124) ? 0 : 1;
}
