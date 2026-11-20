// [driver] An `_Atomic long double` updated in place on x86-64 with
// `cx16` (lock-free at 16 bytes): `x86_fp80` has no same-width integer
// bitcast, so the expected and desired values of the `cmpxchg i128` loop
// go through zero-filled 16-byte temporaries and the old value comes back
// through another, as clang's `convertRValueToInt` /
// `convertAtomicTempToRValue`. It used to be "atomic compare-exchange of
// ... is not supported yet". Pinned to x86-64, no headers. WP-1739.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -mcx16
_Atomic long double x;
void add() { x += 1.0L; }
long double pre() { return ++x; }
long double post() { return x--; }
void scale(long double k) { x *= k; }
