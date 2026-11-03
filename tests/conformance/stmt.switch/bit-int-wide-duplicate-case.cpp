// [stmt.switch]/2: no two case constants of a switch may have the same
// value after conversion to the condition's type, also when the value is
// outside the range of `__int128` (an `unsigned __int128` above its maximum,
// a `_BitInt(N)` with N > 128): each case keeps its exact value, printed in
// decimal. Both cases are located at their value's begin. clang is the
// oracle; x86-64 only (aarch64's `__BITINT_MAXWIDTH__` is 128).
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:8 error "duplicate case value '1461501637330902918203684832716283019655932542976'"
//@ diag: 22:8 note "previous case defined here"
//@ diag: 29:8 error "duplicate case value '340282366920938463463374607431768211455'"
//@ diag: 28:8 note "previous case defined here"
//@ diag: 35:8 error "duplicate case value '8'"
//@ diag: 34:8 note "previous case defined here"
using W = _BitInt(200);
using U = unsigned __int128;
int f(W v) {
  switch (v) {
  case 0:
  case -(W(1) << 160):
  case W(1) << 160:
  case (W(1) << 160) * 1:
    return 1;
  }
  switch (U(v)) {
  case 0:
  case ~U(0):
  case U(-1):
  case U(1) << 127:
    return 2;
  }
  switch (int(v)) {
  case 2 * 4:
  case 1 << 3:
    return 3;
  }
  return 0;
}
