// [expr.comma] (WP-182): clang `CheckCommaOperands` diagnoses a left
// operand whose value is unused: -Wunused-value "left operand of comma
// operator has no effect", the nodiscard / pure / const call forms, and
// -Wunused-comparison for an unparenthesized comparison (a parenthesized
// one is the generic comma warning). Not in an unevaluated operand, a
// for-increment `x, ++i` included; nothing but nodiscard for an operand
// spelled in a macro body. The `=` / `|=` fix-it note follows an unused
// `==` / `!=` whose left operand is an lvalue, built-in or overloaded
// (not a temporary), in an expression statement as well.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 46:5 warning "equality comparison result unused"
//@ diag: 46:5 note "use '=' to turn this equality comparison into an assignment"
//@ diag: 46:17 warning "ignoring return value of function declared with pure attribute"
//@ diag: 46:26 warning "ignoring return value of function declared with const attribute"
//@ diag: 46:35 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 47:5 warning "inequality comparison result unused"
//@ diag: 47:5 note "use '|=' to turn this inequality comparison into an or-assignment"
//@ diag: 47:13 warning "equality comparison result unused"
//@ diag: 47:13 note "use '=' to turn this equality comparison into an assignment"
//@ diag: 47:26 warning "equality comparison result unused"
//@ diag: 47:34 warning "inequality comparison result unused"
//@ diag: 47:34 note "use '|=' to turn this inequality comparison into an or-assignment"
//@ diag: 47:42 warning "equality comparison result unused"
//@ diag: 47:42 note "use '=' to turn this equality comparison into an assignment"
//@ diag: 44:3 warning "left operand of comma operator has no effect"
//@ diag: 44:35 warning "left operand of comma operator has no effect"
//@ diag: 44:55 warning "left operand of comma operator has no effect"
//@ diag: 46:11 warning "left operand of comma operator has no effect"
//@ diag: 46:14 warning "expression result unused"
//@ diag: 46:23 warning "expression result unused"
//@ diag: 46:32 warning "expression result unused"
//@ diag: 46:41 warning "expression result unused"
//@ diag: 46:47 warning "left operand of comma operator has no effect"
//@ diag: 46:54 warning "expression result unused"
//@ diag: 47:19 warning "expression result unused"
//@ diag: 47:50 warning "expression result unused"
__attribute__((pure)) int p(int);
__attribute__((const)) int c(int);
[[nodiscard]] int q(int);
struct M { bool operator==(int) const; bool operator!=(int) const; };
int g(int);
void f(int x, int y, M m) {
  x, g(y); for (int i = 0; i < 2; x, ++i) {} int z = (x, y); (void)z;
  (void)sizeof(x, y); decltype(x, y) w = y; (void)w;
  x == 1; x, 2; p(x), x; c(x), x; q(x), x; (x == 1), x;
  x != 1; m == 1, x; M{} == 1; m != 1; x == 1, x != 1;
}
#define RM(a) (x, a)
#define EQ(a) a == 1
int h(int x) { EQ(x); return RM(1); }
