// The size of __builtin_memcpy_inline is an integer constant expression
// (clang `Builtins.td` `vv*vC*Iz`), and neither it nor
// __builtin_assume_separate_storage is a constant expression (clang 19
// and 22 `ExprConstant` evaluate neither), [expr.const]/10. WP-726.
//@ expect: compile-fail
//@ diag: 12:3 error "argument to '__builtin_memcpy_inline' must be a constant integer"
//@ diag: 17:15 error "static assertion expression is not an integral constant expression"
//@ diag: 18:15 error "static assertion expression is not an integral constant expression"
void f(void *d, const void *s, unsigned long n) {
  __builtin_memcpy_inline(d, s, 4);
  __builtin_memcpy_inline(d, s, sizeof(int) * 2);
  __builtin_memcpy_inline(d, s, n);
}
constexpr int g() { int a = 1, b = 0; __builtin_memcpy_inline(&b, &a, sizeof a); return b; }
constexpr int h() { int a = 1, b = 0; __builtin_assume_separate_storage(&a, &b); return b; }
int i = g() + h();
static_assert(g() == 1);
static_assert(h() == 0);
