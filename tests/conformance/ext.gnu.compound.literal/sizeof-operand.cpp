// GNU: a compound literal after `sizeof` or `alignof` is the operand
// expression, not a parenthesized type-id followed by a stray brace
// (clang's ParseExprAfterUnaryExprOrTypeTrait), and takes its postfix
// suffix; `alignof` of it warns as alignof of any expression.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 12:15 warning "'alignof' applied to an expression is a GNU extension"
struct P { int x, y; };
static_assert(sizeof (P){ 0 } == sizeof(P), "");
static_assert(sizeof (int[]){ 1, 2, 3 } == 3 * sizeof(int), "");
static_assert(sizeof (P){ 1, 2 }.x == sizeof(int), "");
static_assert(alignof (P){ 0 } == alignof(P), "");
int main() { return 0; }
