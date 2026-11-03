// [stmt.expand]/5.1 (P1306R5): each braced element's for-range-declaration
// is constructed in its own block and destroyed at that block's end
// ([stmt.expand]/5: `{ decl = {...}; body }` per element), a temporary
// bound to a reference lives until then ([class.temporary]/6), and an
// element's std::initializer_list backing array is destroyed with it
// ([dcl.init.list]/6). The output is clang 22's for the hand-expanded
// blocks.
//@ expect: run
//@ strict-diags
//@ requires: clang>=23
//@ stdout: S1 b1 ~S1 S23 b23 ~S23 | S4 b4 ~S4 S56 b56 ~S56 | S7 b179 ~S7 S81 b2813 ~S81 | S1 S2 n2 ~S2 ~S1 S3 n1 ~S3 end
#include <cstdio>
#include <initializer_list>
struct S {
  int v;
  S(int v) : v(v) { std::printf("S%d ", v); }
  S(int a, int b) : v(a * 10 + b) { std::printf("S%d ", v); }
  ~S() { std::printf("~S%d ", v); }
};
struct A { int x; S s; int y = 9; };
int main() {
  template for (S s : {{1}, {2, 3}}) { std::printf("b%d ", s.v); }
  std::printf("| ");
  template for (const S& s : {{4}, {5, 6}}) { std::printf("b%d ", s.v); }
  std::printf("| ");
  template for (A a : {{.x = 1, .s = {7}}, {2, {8, 1}, 3}}) { std::printf("b%d%d%d ", a.x, a.s.v, a.y); }
  std::printf("| ");
  template for (auto l : {{S(1), S(2)}, {S(3)}}) { std::printf("n%zu ", l.size()); }
  std::printf("end\n");
}
