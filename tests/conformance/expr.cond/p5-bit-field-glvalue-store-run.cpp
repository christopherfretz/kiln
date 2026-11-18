// [expr.cond]/5: when the second and third operands are glvalues of the
// same type and value category, the result is a glvalue that is a
// bit-field if either operand is one, so an assignment, a compound
// assignment or an increment through `c ? s.p : s.q` stores into the
// selected bit-field only, not the whole storage unit (kiln did that
// before WP-1692; reads were right). clang 22 refuses to generate code
// for such a conditional ("cannot compile this conditional operator
// yet"); compared with GCC 16.1. Found running GCC's C++ testsuite.
// Siblings: p5-bit-field-glvalue-forms-run.cpp (the other forms),
// p5-bit-field-glvalue-constexpr.cpp (constant evaluation).
//@ expect: run
//@ oracle: gcc
//@ skip(clang): clang 19 and 22 do not generate code for a glvalue conditional of bit-fields
//@ stdout: -2 1000
//@ stdout: 5 300 5 7
//@ stdout: 5 77 5 7 99
//@ stdout: -8 80 6 7
//@ stdout: 4
extern "C" int printf(const char *, ...);

struct S { int p : 4; int q : 12; unsigned r : 3; int w; } s = {-2, 1000, 5, 7};
int g = 11;

int rd(bool c) { return c ? s.p : s.q; }
int &rf(bool c, int &o) { return c ? o : g; }
void st(bool c, int v) { (c ? s.p : s.q) = v; }
void mix(bool c, int v) { (c ? s.q : g) = v; }
void add(bool c) { (c ? s.p : s.q) += 3; }
void inc(bool c) { ++(c ? s.r : s.r); }

int main() {
  printf("%d %d\n", rd(true), rd(false));
  st(true, 5);
  st(false, 300);
  printf("%d %d %u %d\n", s.p, s.q, s.r, s.w);
  mix(true, 77);
  mix(false, 99);
  printf("%d %d %u %d %d\n", s.p, s.q, s.r, s.w, g);
  add(true);
  add(false);
  inc(true);
  printf("%d %d %u %d\n", s.p, s.q, s.r, s.w);
  int a = 1;
  rf(true, a) = 4;
  printf("%d\n", a);
}
