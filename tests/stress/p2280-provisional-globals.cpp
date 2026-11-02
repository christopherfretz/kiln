// [expr.const]/10 (C++23, P2280R4) + kiln Oven budgets (spec/oven.md
// "Unknown references"): an `extern` reference with no initializer yet is
// bound to an unknown object afresh by every outermost evaluation (its
// definition may still follow), but to the same Oven object: 10 000
// evaluations naming it hold one static pointer slot, not one each (the
// slot leaked per evaluation before WP-378: static_bytes was 80000).
//@ expect: compile-ok
//@ strict-diags
//@ std: c++23
//@ requires: clang>=22
//@ oven: static_bytes<=16 peak_bytes<=0 allocs<=0 evaluations<=20000
extern const int &r;
extern const int n;
constexpr int k(const int &) { return 1; }

#define A static_assert(k(r) == 1); static_assert(k(n) == 1);
#define B A A A A A A A A A A
#define C B B B B B B B B B B
#define D C C C C C C C C C C
#define E D D D D D D D D D D
E
