// [atomics.types.generic]: on x86-64 a 16-byte `_Atomic` object is not
// lock-free, so the compare-exchange loop of an increment or a compound
// assignment calls `__atomic_compare_exchange` (expected and desired
// materialized in temporaries, x87 padding zeroed); AArch64 keeps the
// `cmpxchg` loop inline.
_Atomic(long double) ld;
_Atomic(__int128) bi;
long double inc() { return ld++; }
long double dec() { return --ld; }
long double scale(long double x) { return ld *= x; }
__int128 mul() { return bi *= 3; }
__int128 div(__int128 d) { return bi /= d; }
