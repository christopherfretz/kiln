// C23 6.4.1, 6.4.2.1: C++'s keywords and alternative tokens ([lex.key],
// [lex.digraph]) are ordinary identifiers in C, and `true` in #if is an
// identifier (so 0) before C23 (6.10.2). restrict and _Bool are C99's.
//@ expect: run
//@ stdout: 46 56 1 1
#include <stdio.h>
#include <stdlib.h>
int class = 1, new = 2, delete = 3, this = 4, template = 5, typename = 6, namespace = 7;
int and = 1, or = 2, not = 0, xor = 3, bitand = 4, compl = 5, and_eq = 6;
int try_(int catch) { return catch; }
static int private, public, protected, virtual, friend, operator, mutable, explicit;
typedef int wchar_t_like; int char16_t = 16, char32_t = 32, char8_t = 8, wchar = 0;
int requires, concept, co_await, co_yield, co_return, consteval, constinit, decltype, noexcept, using, typeid, throw, export;
#define and_def and
#if true
#error true is not 1 before C23
#endif
int main(void) {
  _Bool b = 1;
  int *restrict p = &class;
  int x = class + new + delete + this + template + typename + namespace + (and & or) + not + xor + bitand + compl + and_eq;
  printf("%d %d %d %d\n", x, try_(char16_t + char32_t + char8_t), (int)b, *p);
  return 0;
}
