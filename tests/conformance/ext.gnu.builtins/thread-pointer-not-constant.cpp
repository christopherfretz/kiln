// WP-782: `__builtin_thread_pointer()` is never a constant expression.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:17 error "constexpr variable 'p' must be initialized by a constant expression"
//@ diag: 7:31 note "subexpression not valid in a constant expression"
//@ diag: 8:21 note "in call to 'tp()'"
constexpr void *tp() { return __builtin_thread_pointer(); }
constexpr void *p = tp();
