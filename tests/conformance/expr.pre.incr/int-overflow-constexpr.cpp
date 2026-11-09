// [expr.pre.incr]/1, [expr.const]/5.8: `++`/`--` on `int`, `long` and
// `long long` is computed in that type, so stepping past its range is
// undefined and not a constant expression; only types narrower than
// `int` wrap through the promotion (narrow-signed-wrap-constexpr.cpp,
// WP-980).
//@ expect: compile-fail
//@ std: c++26
//@ strict-diags
//@ diag: 22:15 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 21:45 note "value 2147483648 is outside the range of representable values of type 'int'"
//@ diag: 22:19 note "in call to 'i_inc()'"
//@ diag: 24:15 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 23:51 note "value -2147483649 is outside the range of representable values of type 'int'"
//@ diag: 24:19 note "in call to 'i_dec()'"
//@ diag: 26:16 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 25:50 note "value 9223372036854775808 is outside the range of representable values of type 'long'"
//@ diag: 26:20 note "in call to 'l_inc()'"
//@ diag: 28:21 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 27:70 note "value -9223372036854775809 is outside the range of representable values of type 'long long'"
//@ diag: 28:25 note "in call to 'll_dec()'"
constexpr int i_inc() { int x = 2147483647; ++x; return x; }
constexpr int a = i_inc();
constexpr int i_dec() { int x = -2147483647 - 1; x--; return x; }
constexpr int b = i_dec();
constexpr long l_inc() { long x = __LONG_MAX__; x++; return 0; }
constexpr long c = l_inc();
constexpr long long ll_dec() { long long x = -__LONG_LONG_MAX__ - 1; --x; return 0; }
constexpr long long d = ll_dec();
