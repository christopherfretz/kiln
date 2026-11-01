// [expr.const]/10: a checked arithmetic builtin in a constant expression
// stores through its result pointer, so a null pointer or an object
// created outside the evaluation makes it non-constant; the library
// builtin __builtin_vsnprintf is never constant. Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:15 error "static assertion expression is not an integral constant expression"
//@ diag: 18:51 note "assignment to dereferenced null pointer is not allowed in a constant expression"
//@ diag: 19:15 note "in call to 'g1()'"
//@ diag: 22:15 error "static assertion expression is not an integral constant expression"
//@ diag: 21:30 note "a constant expression cannot modify an object that is visible outside that expression"
//@ diag: 22:15 note "in call to 'g4()'"
//@ diag: 29:15 error "static assertion expression is not an integral constant expression"
//@ diag: 27:10 note "subexpression not valid in a constant expression"
//@ diag: 29:15 note "in call to 'g6()'"
//@ diag: 30:15 error "static assertion expression is not an integral constant expression"
//@ diag: 30:15 note "assignment to dereferenced null pointer is not allowed in a constant expression"
constexpr int g1() { int r = 0; int *p = nullptr; __builtin_add_overflow(1, 2, p); return r; }
static_assert(g1() == 0);
int glob;
constexpr bool g4() { return __builtin_add_overflow(1, 2, &glob); }
static_assert(g4());
constexpr bool g6() {
  const char *s = "x";
  char buf[4];
  __builtin_va_list ap;
  return __builtin_vsnprintf(buf, 4, s, ap);
}
static_assert(g6());
static_assert(__builtin_add_overflow(1, 2, (int *)nullptr) == 0);
