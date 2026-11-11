// WP-1288: `__builtin_extract_return_addr` is never a constant expression.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:17 error "constexpr variable 'q' must be initialized by a constant expression"
//@ diag: 8:14 note "subexpression not valid in a constant expression"
//@ diag: 10:21 note "in call to 'f(nullptr, true)'"
constexpr void *f(void *p, bool b) {
  return b ? __builtin_extract_return_addr(p) : p;
}
constexpr void *q = f(nullptr, true);
