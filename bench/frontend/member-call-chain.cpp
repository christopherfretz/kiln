//@ generate: call-chain 5000
// Long postfix chains ([expr.ref], [expr.call]; WP-2201), generated at
// bench time (spec/bench.md "Generated entries"): three expression
// statements of 5,000 member calls each, `x.f().f()...` returning a
// reference, `p->g()->g()...`, and `v.h().h()...` returning by value a
// class with a non-trivial destructor (a temporary per call). Code
// generators emit such chains (builders, fluent interfaces). Measures the
// per-call cost of member access, call building and the full-expression
// checks: before WP-2201 every call walked the chain below it to find
// where it begins, so the chain cost O(N^2). The generator is
// xtask/src/bench/generate.rs.
