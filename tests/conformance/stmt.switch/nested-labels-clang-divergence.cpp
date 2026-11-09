// [stmt.switch]/5, [stmt.dcl]/3, [stmt.for]/1 (WP-1144): two jumps to a
// nested label where clang's constant evaluator (19 and 22, ExprConstant.cpp
// `EvaluateStmt` with a `Case` to hunt) differs from the program's run-time
// behaviour, which kiln follows:
// - a block-scope class object with vacuous initialization (a trivial
//   default constructor) may be bypassed, and is usable after the label;
//   clang starts the lifetime only of a bypassed variable with no
//   initializer at all, so `p.a = 6` fails there;
// - after the jump into a `for` body, the loop goes on with its increment
//   and condition; clang then evaluates the loop again from the start,
//   running its init-statement a second time.
// The asserted values are the run-time results (clang++-19, -O0).
//@ skip(clang): clang's constant evaluator diverges from the run-time results here (see above)
//@ strict-diags
struct P {
  int a;
};
constexpr int bypass_class(int x) {
  int r = 0;
  switch (x) {
  {
    P p;
  case 1:
    p.a = 6;
    r += p.a;
  }
  }
  return r;
}
static_assert(bypass_class(1) == 6);
static_assert(bypass_class(2) == 0);

constexpr int for_init(int x) {
  int r = 0, k = 0;
  switch (x) {
  case 0:
    for (k = 0; k < 3; ++k) {
      r += 1;
    case 1:
      r += 10;
    }
  }
  return r * 10 + k;
}
static_assert(for_init(0) == 333);
static_assert(for_init(1) == 323);
static_assert(for_init(2) == 0);
