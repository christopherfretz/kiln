// [c.math.syn], [cpp.cond]: at -fclang-compat=19 fmaximum_num /
// fminimum_num and sincos, which only clang 22 declares, are not builtins
// (an undeclared identifier). At 22: c-math-builtins-22.cpp (WP-410).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 17:9 error "use of undeclared identifier '__builtin_fmaximum_num'"
//@ diag: 18:9 error "use of undeclared identifier '__builtin_fminimum_numf'"
//@ diag: 19:3 error "use of undeclared identifier '__builtin_sincos'"
static_assert(!__has_builtin(__builtin_fmaximum_num) && !__has_builtin(__builtin_fmaximum_numf) &&
              !__has_builtin(__builtin_fmaximum_numl));
static_assert(!__has_builtin(__builtin_fminimum_num) && !__has_builtin(__builtin_fminimum_numl));
static_assert(!__has_builtin(__builtin_sincos) && !__has_builtin(__builtin_sincosf) &&
              !__has_builtin(__builtin_sincosl));

void f(double d, float x, double *p) {
  (void)__builtin_fmaximum_num(d, d);
  (void)__builtin_fminimum_numf(x, x);
  __builtin_sincos(d, p, p);
}
