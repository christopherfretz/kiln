// [expr.pre]/4, [expr.shift]/2, [expr.mul]/4: undefined __int128
// operations are not constant expressions; clang's notes.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:15 error "static assertion expression is not an integral constant expression"
//@ diag: 13:59 note "value 170141183460469231731687303715884105728 is outside the range of representable values of type '__int128'"
//@ diag: 16:15 note "in call to 'div(-170141183460469231731687303715884105728, -1)'"
//@ diag: 17:15 error "static assertion expression is not an integral constant expression"
//@ diag: 14:54 note "shift count 128 >= width of type '__int128' (128 bits)"
//@ diag: 17:15 note "in call to 'shl(1, 128)'"
//@ diag: 18:15 error "static assertion expression is not an integral constant expression"
//@ diag: 18:15 note "value 170141183460469231731687303715884105728 is outside the range of representable values of type '__int128'"
constexpr __int128 div(__int128 a, __int128 b) { return a / b; }
constexpr __int128 shl(__int128 a, int n) { return a << n; }
constexpr __int128 imin = (__int128)((unsigned __int128)1 << 127);
static_assert(div(imin, -1) == 0);
static_assert(shl(1, 128) == 0);
static_assert(-imin == 0);
