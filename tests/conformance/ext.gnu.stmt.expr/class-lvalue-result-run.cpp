// A statement expression whose last statement names a class lvalue (a local or a global) is a
// prvalue copy-initialized from it, inside the statement expression's scope: the copy is made
// before the local is destroyed, and a class result initializes its destination directly
// (OpenCV's carotene and NEON helpers return `({ T r; ...; r; })`).
//@ expect: run
//@ stdout: c3 cc13 d3 c5 cc15 d5 | 1 2 2 13 15 d15 d13 end
#include <cstdio>
struct V { int a, b; };
struct T {
  int v;
  T(int x) : v(x) { std::printf("c%d ", v); }
  T(const T& o) : v(o.v + 10) { std::printf("cc%d ", v); }
  ~T() { std::printf("d%d ", v); }
};
static V g{1, 2};
#define LD(p) ({ V r_ = *(p); r_; })
#define GT(x) ({ T t_(x); t_; })
static void run() {
  V a = LD(&g);
  V b = ({ g; });
  const T& r = GT(3);
  std::printf("| %d %d %d %d %d ", a.a, a.b, b.b, r.v, ({ T u(5); u; }).v);
}
int main() {
  run();
  std::printf("end\n");
}
