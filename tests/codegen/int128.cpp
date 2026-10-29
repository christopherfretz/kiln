// __int128 parameters, returns and arithmetic: two i64 halves on x86-64
// (through coercion temporaries), i128 on AArch64.
__int128 mul128(__int128 a, __int128 b) { return a * b; }
unsigned __int128 shl128(unsigned __int128 a, int n) { return a << n; }
long low(__int128 v) { return (long)v; }
long i128(long x) { return low(mul128(x, 3)) + (long)shl128(1, 70 - 64); }
