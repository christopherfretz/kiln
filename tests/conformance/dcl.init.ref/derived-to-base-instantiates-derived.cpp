// [dcl.init.ref]/4, [temp.inst]/2: whether `S<T>` is a base of the
// initializer's class needs that class complete, so binding a reference
// to a base class instantiates a class template specialization nothing
// else has completed (here only named through reference and pointer
// parameters): in a declaration's initializer, through overload
// resolution ([over.best.ics]/6), and through static_cast
// ([expr.static.cast]/2). Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 3 4 5
#include <stdio.h>
template<class T> struct S { int v; };
template<class T> struct D : S<T> {};

int direct(D<int> &d) {
  S<int> &r = d;
  return r.v;
}
int through_ptr(D<char> *d) {
  const S<char> &r = *d;
  return r.v;
}
int pick(S<short> &s) { return s.v; }
int pick(long) { return -1; }
int overload(D<short> &d) { return pick(d); }
int cast(D<long> &d) { return static_cast<S<long> &>(d).v; }
int rvalue(D<unsigned> &d) {
  S<unsigned> &&r = static_cast<D<unsigned> &&>(d);
  return r.v;
}

int main() {
  D<int> a;
  a.v = 1;
  D<char> b;
  b.v = 2;
  D<short> c;
  c.v = 3;
  D<long> e;
  e.v = 4;
  D<unsigned> f;
  f.v = 5;
  printf("%d %d %d %d %d\n", direct(a), through_ptr(&b), overload(c), cast(e), rvalue(f));
}
