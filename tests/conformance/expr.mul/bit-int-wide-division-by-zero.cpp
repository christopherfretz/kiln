// [expr.mul]/4: division or remainder by a zero divisor is undefined;
// -Wdivision-by-zero warns of a constant zero divisor also when it is a
// `_BitInt(N)` with N > 128 (or the operation's type is), as clang does.
// clang is the oracle; x86-64 only (aarch64's `__BITINT_MAXWIDTH__` is 128).
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
//@ diag: 12:24 warning "division by zero is undefined"
//@ diag: 12:35 warning "remainder by zero is undefined"
//@ diag: 13:25 warning "division by zero is undefined"
//@ diag: 14:27 warning "remainder by zero is undefined"
using W = _BitInt(200);
int dz(W a) { return a / W(0) + a % (W)0; }
int dz2(W a) { return a / 0; }
int dz3(int a) { return a % (W(1) << 160 >> 170); }
int ok(W a) { return a / (W(1) << 150); }
