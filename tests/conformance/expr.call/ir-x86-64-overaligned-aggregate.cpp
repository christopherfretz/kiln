// [expr.call]/7, [dcl.align]/2: on x86-64 `alignas` changes the parameter
// classification only through the size it implies (spec/abi.md §8.12,
// spec/landed/abi/WP-583.md). An `alignas(32)` struct of one `long` is 32
// bytes, so it is MEMORY (`byval` with align 32), while an `alignas(16)`
// pair of `long`s stays two INTEGER eightbytes, as does its return.
// Compared with clang's IR exactly.
//@ expect: ir
//@ requires: clang>=22
//@ args: --target=x86_64-unknown-linux-gnu -ftrivial-auto-var-init=uninitialized
struct alignas(32) X { long v; };
struct alignas(16) A { long lo, hi; };
static_assert(sizeof(X) == 32);

long fx(int i, X x) { return i + x.v; }
long fa(int i, A a) { return i + a.lo + a.hi; }
A ra(long l) { return A{l, l + 1}; }
long calls() { return fx(1, X{2}) + fa(3, A{4, 5}) + ra(6).hi; }
