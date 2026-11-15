// C17 6.6p3, p6: an integer constant expression (an enumerator, a `case`
// label, a `_Static_assert` condition) whose operand reads an object or
// calls a function is not one. clang (CheckICE) reports it at the first
// such operand, in source order, with no evaluation notes; `sizeof` operands
// are not looked into. Reduced from WP-1438's list; WP-1435.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=gnu17
//@ diag: 20:17 error "expression is not an integer constant expression"
//@ diag: 21:17 error "expression is not an integer constant expression"
//@ diag: 22:24 error "expression is not an integer constant expression"
//@ diag: 23:14 error "expression is not an integer constant expression"
//@ diag: 24:20 error "static assertion expression is not an integral constant expression"
//@ diag: 25:30 error "static assertion expression is not an integral constant expression"
//@ diag: 28:12 error "expression is not an integer constant expression"
//@ diag: 30:26 error "expression is not an integer constant expression"
int g;
int h(void);
struct S { int a[4]; int m; } s;
enum { E1 = 2 + g };
enum { E2 = 1 + h() };
enum { E3 = sizeof g + s.m };
enum { E4 = (g ? 1 : 2) + g };
_Static_assert(1 + g, "x");
_Static_assert(3 * (1 + 2) - *&g, "y");
void f(int x) {
  switch (x) {
  case 3 * g:
    break;
  case 4 + sizeof(int) + x:
    break;
  }
}
