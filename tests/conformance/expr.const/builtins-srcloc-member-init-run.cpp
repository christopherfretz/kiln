// [support.srcloc.cons]/2, [class.base.init]/9, [dcl.init.aggr]/5: in a
// default member initializer the location builtins (and
// source_location::current()) take the use: the constructor's name, or the
// closing brace of an aggregate initialization, and its function.
//@ expect: run
//@ stdout: U 23 3 | 24 1 | 28 4
//@ stdout: Agg 40 10 | 42 7 | 43 11
//@ stdout: Outer 15 8 S 15 30
//@ stdout: TS 31 TS | 47 main
//@ stdout: tf 32 tf 32
//@ stdout: gs 33 6 '' 33
//@ stdout: new 15 8 S | 15 8
#include <source_location>
extern "C" int printf(const char *, ...);
struct S {
  int line = __builtin_LINE();
  unsigned col = __builtin_COLUMN();
  const char *fn = __builtin_FUNCTION();
  std::source_location loc = std::source_location::current();
};
struct U {
  int line = __builtin_LINE(); unsigned col = __builtin_COLUMN();
  U() {}
  U(int)
    : col(1) {}
  U(char);
};
U::U(char) {}
struct Agg { int a; int line = __builtin_LINE(); unsigned col = __builtin_COLUMN(); };
struct Outer { S s; int z = __builtin_LINE(); Outer() = default; };
template <class T> struct TS { int line = __builtin_LINE(); const char *fn = __builtin_FUNCTION(); };
template <class T> void tf() { S s{}; printf("tf %d %s %u\n", s.line, s.fn, s.loc.line()); }
S gs{};
constexpr S cs{};
static_assert(cs.line == 34);
static_assert(cs.loc.line() == 34);
int main() {
  U u0; U u1(1); U u2('c');
  printf("U %d %u | %d %u | %d %u\n", u0.line, u0.col, u1.line, u1.col, u2.line, u2.col);
  Agg g{1};
  Agg g2 = {
    2 };
  Agg g3{3};
  printf("Agg %d %u | %d %u | %d %u\n", g.line, g.col, g2.line, g2.col, g3.line, g3.col);
  Outer o;
  printf("Outer %d %u %s %u %d\n", o.s.line, o.s.col, o.s.fn, o.s.loc.line(), o.z);
  TS<int> t; TS<int> t2{};
  printf("TS %d %s | %d %s\n", t.line, t.fn, t2.line, t2.fn);
  tf<int>();
  printf("gs %d %u '%s' %u\n", gs.line, gs.col, gs.fn, gs.loc.line());
  S *p = new S;
  S *q = new S();
  printf("new %d %u %s | %d %u\n", p->line, p->col, p->fn, q->line, q->col);
  constexpr S ls{};
  static_assert(ls.line == 54 && ls.col == 18);
}
