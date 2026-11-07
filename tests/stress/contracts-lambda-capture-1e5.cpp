// spec/contracts.md §5 and §6.4 ([expr.prim.lambda.capture]/3.3,
// [basic.contract.eval], P2900R14): a hundred thousand constant-evaluated
// calls of a function whose precondition and postcondition each call a
// lambda capturing the function's parameter (by copy) and the result
// binding (by reference). Pins that a capturing lambda in a predicate
// costs a bounded number of steps per call (two lambda calls, 7 steps a
// call against 3 without the contracts) and no memory per call: the
// closure objects are the predicate's temporaries (32 bytes a call) and
// are reclaimed at once, so the peak stays at one closure whatever the
// count. With -DNO_CONTRACTS the same code has no contracts.
// WP-755 measured (release kiln, -fsyntax-only, best of 3): enforce and
// observe 0.05 s / 700,005 steps, ignore and without contracts 0.02 s /
// 300,005 steps, all 12-13 MiB RSS. GCC 16.1 rejects naming a parameter
// in such a lambda (spec/contracts.md §9), so there is no oracle.
//@ expect: compile-ok
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ args: -fcontract-evaluation-semantic=enforce -fconstexpr-steps=10000000
//@ oven: evaluations<=1 calls<=300001 steps<=720000 allocs<=0 peak_bytes<=64
//@ mem<=256
#ifdef NO_CONTRACTS
#define PRE(...)
#define POST(r, ...)
#else
#define PRE(...) pre(__VA_ARGS__)
#define POST(r, ...) post(r : __VA_ARGS__)
#endif
constexpr int step(const int x)
  PRE([x] { return x >= 0; }()) POST(r, [&r, x] { return r > x; }()) {
  return x + 1;
}
consteval int run(int n) {
  int x = 0;
  for (int i = 0; i < n; ++i) x = step(x);
  return x;
}
static_assert(run(100000) == 100000);
