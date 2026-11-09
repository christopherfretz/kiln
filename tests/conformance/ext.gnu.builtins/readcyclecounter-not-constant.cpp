// WP-1057: `__builtin_readcyclecounter()` is never a constant expression.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:30 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 7:44 note "subexpression not valid in a constant expression"
//@ diag: 8:34 note "in call to 'rc()'"
constexpr unsigned long long rc() { return __builtin_readcyclecounter(); }
constexpr unsigned long long c = rc();
