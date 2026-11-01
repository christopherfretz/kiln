// [atomics.types.generic] support: _Atomic objects wider than the target's
// inline atomic width (x86-64: 16-byte long double and __int128) load and
// store through the __atomic_load/__atomic_store libcalls; AArch64 keeps
// them inline.
_Atomic(long double) ld;
_Atomic(__int128) bi;
long double f() { ld = 1.0L; return ld; }
__int128 g() { bi = 3; return bi; }
