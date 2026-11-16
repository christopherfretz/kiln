// [dcl.init.aggr]/5.2: omitted elements that value-initialize (a
// default constructor, a defaulted argument, a scalar) take no note; the
// sibling omitted-init-note shows the note when one fails.
//@ std: c++20
//@ expect: run
//@ stdout: 7 1 7 7 3 0 0 0
#include <cstdio>
struct D { D(int v = 7) : v(v) {} int v; };
struct E { int i; D d; };
struct A { int &r; int k; };
int x = 3;
struct C { A a; int i; };
int main() {
  E e = {1};
  D *p = new D[3]{1};
  int n = 2;
  D *q = new D[n]{5};
  C c = {{x}};
  int arr[3] = {1};
  std::printf("%d %d %d %d %d %d %d %d\n", e.d.v, p[0].v, p[2].v, q[1].v, c.a.r, c.a.k, c.i, arr[2]);
  delete[] p;
  delete[] q;
}
