//@ args: -mno-outline-atomics
// WP-226: -mno-outline-atomics puts -outline-atomics in every function's
// "target-features" on AArch64 (after -fmv, as clang 19 orders it); on
// x86-64 the flag is ignored (clang warns -Woption-ignored). [atomics.ref.ops]
_Atomic(int) gi;
int add(int v) { return __c11_atomic_fetch_add(&gi, v, 5); }
