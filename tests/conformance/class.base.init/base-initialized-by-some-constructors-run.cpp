// [class.base.init]/6, /9, /13: which constructor of a
// class runs decides which of its bases are initialized by a
// mem-initializer and which are default-initialized; a delegating
// constructor initializes no base itself (its target does). The Oven
// marks a base no constructor initializes as outside its lifetime
// (WP-390, class.base.init/unconstructed-base-constexpr.cpp); this checks
// the valid neighbours of that rule: every value read at compile time
// equals the one read at run time.
//@ expect: run
//@ stdout: 8 8 | 9 9 | 13 13 | 30 30
#include <cstdio>
struct A { int a = 1; };
struct B : A {
  int x;
  constexpr B() : x(2) {}
  constexpr B(int v) : x(v) {}
  constexpr int get() const { return a + x; }
};
struct C { int c; constexpr C() : c(5) {} };
struct D : C, B {
  int y;
  constexpr D() : y(0) {}
  constexpr D(char) : D() { y = 1; }
  constexpr D(int v) : B(v), y(4) {}
  constexpr D(int v, int w) : D(v + w) { y = w; }
};
constexpr int f1() { D d; return d.get() + d.c + d.y; }
constexpr int f2() { D d('a'); return d.get() + d.c + d.y; }
constexpr int f3() { D d(3); return d.get() + d.c + d.y; }
constexpr int f4() { D d(3, 6); const B &b = d; return b.get() + d.c + d.x + d.y; }
int main() {
  constexpr int c1 = f1(), c2 = f2(), c3 = f3(), c4 = f4();
  volatile int r1 = f1(), r2 = f2(), r3 = f3(), r4 = f4();
  std::printf("%d %d | %d %d | %d %d | %d %d\n", c1, (int)r1, c2, (int)r2, c3, (int)r3, c4, (int)r4);
}
