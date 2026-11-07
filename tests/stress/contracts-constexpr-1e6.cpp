// spec/contracts.md §6.4 (P2900R14 [basic.contract.eval]): one million
// constant-evaluated calls of a function with a precondition and a
// postcondition naming the result. Pins that checking a contract in the
// Oven costs a bounded number of steps per call (the predicates are
// compiled once, with the function) and no memory per call: the frame,
// the result object the postcondition reads in place, and the predicates'
// temporaries are reclaimed as for any call. With -DNO_CONTRACTS the same
// code has no contracts; under ignore the predicates are skipped.
// WP-707 measured (release kiln, -fsyntax-only, best of 3): enforce
// 0.15 s, ignore 0.13 s, without contracts 0.14 s, observe (each
// predicate a trial) 0.23 s, all 13 MiB RSS and 3,000,005 steps. GCC
// 16.1 needs -fconstexpr-loop-limit and takes 1.8 s / 198 MiB (1.7 s /
// 196 MiB without contracts), so there is no oracle.
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
