//@ expect: run
// [dcl.init.general]/16.5, /16.6.2.2, /9 (P0960; WP-182): parenthesized
// initialization of multidimensional arrays of classes. A braced-init-list
// argument copy-list-initializes its row; the remaining rows (and a
// value-initialized member that is an array of classes) run the default
// constructor for every element; destructors run for every element.
#include <cstdio>
static int live = 0;
struct S {
  S() : v(7) { ++live; }
  S(int x) : v(x) { ++live; }
  S(const S& o) : v(o.v) { ++live; }
  ~S() { --live; }
  int v;
};
struct U { U() : v(9) {} U(int x) : v(x) {} int v; };
struct D { int x; S s[2]; };
struct E { E() : s() {} S s[2][2]; };
U gu[2][2]({U(5)});
int gi[2][3]({1, 2}, {3});
int main() {
  {
    S a[3][2]({S(1), S(2)}, {S(3)});
    std::printf("%d %d %d %d %d %d live=%d\n", a[0][0].v, a[0][1].v, a[1][0].v, a[1][1].v,
                a[2][0].v, a[2][1].v, live);
    D d(4);
    std::printf("%d %d %d live=%d\n", d.x, d.s[0].v, d.s[1].v, live);
    E e;
    std::printf("%d live=%d\n", e.s[1][1].v, live);
  }
  std::printf("live=%d\n", live);
  U (*p)[2] = new U[2][2]({U(4)});
  std::printf("%d %d %d %d\n", p[0][0].v, p[0][1].v, p[1][0].v, p[1][1].v);
  delete[] p;
  int (*q)[2] = new int[3][2]({1, 2}, {3});
  std::printf("%d %d %d %d\n", q[0][1], q[1][0], q[1][1], q[2][1]);
  delete[] q;
  std::printf("%d %d %d %d %d\n", gu[0][0].v, gu[0][1].v, gu[1][0].v, gi[0][1], gi[1][2]);
}
