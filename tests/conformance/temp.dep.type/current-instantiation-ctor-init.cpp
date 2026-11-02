// [temp.dep.type]/7, [dcl.init.general]/16.6: a variable whose type is the
// (dependent) current instantiation is initialized only at instantiation;
// overload resolution against a dependent class type must not run at
// definition time, where a user-declared copy constructor made `A a(c)`
// ambiguous. libc++ test/support/container_test_types.h:324
// (InAllocatorConstructGuard; map index_key, unord.map index, vector
// construct_iter_iter_alloc, WP-276). Verified against clang 19.
//@ expect: run
//@ stdout: 1 2 3 4 5 6 7 8
#include <cstdio>
struct CC { int v; };
template <class T> struct A {
  A(CC *c) : v(c->v) {}
  A(A const &o) : v(o.v) {}
  int v;
  CC *c;
  static int take(A a) { return a.v; }
  int d1() { A a(c); return a.v; }
  int d2() { A a = c; return a.v; }
  A d3() { return c; }
  int d4() { return take(c); }
  int d5() { A a = A(c); return a.v; }
  int d6() { return static_cast<A>(c).v; }
  int d7() { A *p = new A(c); int r = p->v; delete p; return r; }
  int d8() { A a{c}; return a.v; }
};
int main() {
  CC cs[8];
  int r[8];
  for (int i = 0; i < 8; ++i) cs[i].v = i + 1;
  A<int> x(&cs[0]);
  x.c = &cs[0]; r[0] = x.d1();
  x.c = &cs[1]; r[1] = x.d2();
  x.c = &cs[2]; r[2] = x.d3().v;
  x.c = &cs[3]; r[3] = x.d4();
  x.c = &cs[4]; r[4] = x.d5();
  x.c = &cs[5]; r[5] = x.d6();
  x.c = &cs[6]; r[6] = x.d7();
  x.c = &cs[7]; r[7] = x.d8();
  std::printf("%d %d %d %d %d %d %d %d\n", r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7]);
}
