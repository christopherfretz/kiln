// [dcl.constexpr]/6: a builtin clang never evaluates in a constant
// expression (no `Constexpr` attribute in Builtins.td), called with known
// arguments, still makes the function never constant; clang 19 reports
// each (-Winvalid-constexpr, an error by default before C++23). The
// WP-962 conservative rule keeps these true positives.
//@ std: c++20
//@ strict-diags
//@ expect: compile-fail
//@ diag: 15:23 error "constexpr function never produces a constant expression"
//@ diag: 15:51 note "subexpression not valid in a constant expression"
//@ diag: 16:17 error "constexpr function never produces a constant expression"
//@ diag: 16:37 note "subexpression not valid in a constant expression"
//@ diag: 17:18 error "constexpr function never produces a constant expression"
//@ diag: 17:45 note "subexpression not valid in a constant expression"
constexpr const char *tp1(const char *p) { return __builtin_strstr("ab", "b"); }
constexpr void *tp2(int x) { return __builtin_alloca(4); }
constexpr double tp3(double x) { return x + __builtin_pow(2.0, 3.0); }
