// spec/contracts.md §6.4 (P2900R14 [basic.contract.eval]): one million
// constant-evaluated calls of a function with a precondition and a
// postcondition naming the result. Pins that checking a contract in the
// Oven costs a bounded number of steps per call (the predicates are
// compiled once, with the function) and no memory per call: the frame,
// the result object the postcondition reads in place, and the predicates'
// temporaries are reclaimed as for any call. With -DNO_CONTRACTS the same
// code has no contracts; under ignore the predicates are skipped.
// WP-707 measured (release kiln, -fsyntax-only, best of 3): enforce
// 0.43 s / 15 MiB RSS, ignore 0.31 s / 15 MiB, without contracts
// 0.31 s / 15 MiB; GCC 16.1 at enforce needs a raised
// -fconstexpr-ops-limit and ~2 s, so there is no oracle.
//@ expect: compile-ok
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ args: -fcontract-evaluation-semantic=enforce -fconstexpr-steps=10000000
//@ oven: evaluations<=1 calls<=1000001 steps<=3100000 allocs<=0 peak_bytes<=0
//@ mem<=256
#ifdef NO_CONTRACTS
#define PRE(e)
#define POST(r, e)
#else
#define PRE(e) pre(e)
#define POST(r, e) post(r : e)
#endif
constexpr int step(const int x) PRE(x >= 0) POST(r, r > x) { return x + 1; }
consteval int run(int n) {
  int x = 0;
  for (int i = 0; i < n; ++i) x = step(x);
  return x;
}
static_assert(run(1000000) == 1000000);
