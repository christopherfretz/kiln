// [dcl.init.general]/16.6.2.2, [dcl.init.aggr]/5.1, [support.srcloc.cons]/2,
// [temp.inst]: a parenthesized aggregate initialization in a function
// template whose aggregate has a default member initializer naming the
// function (`__builtin_FUNCTION()`) is formed again in each specialization:
// its written arguments are substituted, and the trailing members it fills
// (the default member initializers, the value-initialized members) are
// filled again in the specialization's own context, never substituted as
// if written. A braced argument (`B b({N, 1}, {N, 2})`) stays a braced
// argument. Before WP-1740 both stopped with "not yet implemented:
// substitution into CXXDefaultInitExpr" / "InitListExpr".
//@ expect: run
//@ strict-diags
//@ stdout: paren_default paren_default paren_default 13 9 4
#include <cstdio>
struct D { int a; const char* s = __builtin_FUNCTION(); int z = 7; };
struct A { int x; int y; };
struct B { A a; A b; };
struct C { int x; A a; int z; };
template <int N> int paren_default() {
  D d(N);
  D e = D(N + 1);
  D* p = new D(N + 2);
  std::printf("%s %s %s ", d.s, e.s, p->s);
  int r = d.a + e.a + p->a + d.z;
  delete p;
  return r;
}
template <int N> int paren_braced() {
  B b({N, 1}, {N, 2});
  A arr[2]({N, 3});
  return b.a.x + b.b.y + arr[0].y + arr[1].x;
}
template <int N> int paren_value_init() {
  C c(N);
  C* p = new C(N + 1, {N, N});
  int r = c.x + c.a.x + c.a.y + c.z + p->x + p->a.y;
  delete p;
  return r;
}
int main() {
  int r = paren_default<1>();
  std::printf("%d ", r);
  r = paren_braced<4>();
  std::printf("%d ", r);
  r = paren_value_init<1>();
  std::printf("%d\n", r);
}
