// [stmt.stmt], [stmt.return]/4, spec/sema.md §7.5.1: one function body of
// 9,990 statements (if/else, switch with break, while, for with
// continue, goto, blocks with destructors), expanded from nested macros.
// -Wreturn-type, -Wimplicit-fallthrough and -Wunreachable-code all query
// the body's one control-flow graph; the budget pins that building it
// stays linear. The body can fall off its end on one path only.
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wimplicit-fallthrough -Wunreachable-code
//@ diag: 31:1 warning "non-void function does not return a value in all control paths"
//@ mem<=512
struct D { ~D(); };
int g(int);
#define U(n)                                                                   \
  x += g(n);                                                                   \
  if (x > n) x -= 3; else { D d; x += 2; }                                     \
  switch (x & 3) { case 0: x++; break; case 1: x--; break; default: break; }   \
  while (x > 1000000) x /= 2;                                                  \
  for (int i = 0; i < (x & 7); ++i) { if (i == n) continue; x ^= i; }          \
  if (x == -n) goto L##n;                                                      \
  x = g(x);                                                                    \
  L##n:;
#define A(p) U(p##0) U(p##1) U(p##2) U(p##3) U(p##4) U(p##5) U(p##6) U(p##7) U(p##8) U(p##9)
#define B(p) A(p##0) A(p##1) A(p##2) A(p##3) A(p##4) A(p##5) A(p##6) A(p##7) A(p##8) A(p##9)
// 333 expansions of U, 30 statements each.
int big(int x) {
  B(10) B(11) B(12)
  A(130) A(131) A(132)
  U(1330) U(1331) U(1332)
  if (x == 0) return 1;
}
