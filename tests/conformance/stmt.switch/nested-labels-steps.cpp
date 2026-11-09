// [stmt.switch]/2, [expr.const]/10 (WP-1144): the jump to a label nested in
// a loop body charges clang's steps for the label hunt (`EvaluateStmt` with
// a `Case`: one per statement visited), so a Duff's device evaluation needs
// exactly clang's step budget: 32 for `duff(9)` (31 fails in both).
//@ args: -fconstexpr-steps=32
//@ strict-diags
// Duff's device.
constexpr int duff(int count) {
  int sum = 0, k = 0;
  int n = (count + 3) / 4;
  switch (count % 4) {
  case 0: do { sum += ++k;
  case 3:      sum += ++k * 2;
  case 2:      sum += ++k * 3;
  case 1:      sum += ++k * 4;
          } while (--n > 0);
  }
  return sum;
}
static_assert(duff(9) == 124);
