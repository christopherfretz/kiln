// [expr.const]/10 (C++23, P2280R4) + kiln Oven budgets (spec/oven.md
// "Unknown references"): the unspecified object an unknown reference binds
// to, and the object of an `extern` variable, hold no bytes in the Oven:
// naming a reference to a 2 GB array, or an `extern` array of 2 GB, costs
// a slot, not its size.
//@ expect: compile-ok
//@ strict-diags
//@ std: c++23
//@ requires: clang>=22
//@ oven: 17 static_bytes<=64 peak_bytes<=64
//@ oven: 21 static_bytes<=64
//@ mem<=256
using size_t = decltype(sizeof 0);
template <class T, size_t N> constexpr size_t size(T (&)[N]) { return N; }

void huge(char (&a)[2000000000]) {
  static_assert(size(a) == 2000000000);
}

extern char big[2000000000];
static_assert(&big[1] - big == 1);
