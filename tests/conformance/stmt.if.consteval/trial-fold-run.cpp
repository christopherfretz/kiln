// [stmt.if]/4, [expr.const]/21: `if consteval` takes its compound
// statement only in a manifestly constant-evaluated evaluation. A call the
// compiler folds on its own (a plain `if` condition, an operand of `&&`,
// a `?:`, a non-constant initializer) is not one: it takes the else branch
// (WP-588).
//@ expect: run
constexpr bool ice() { if consteval { return true; } else { return false; } }
constexpr bool nice() { if !consteval { return false; } else { return true; } }
inline int f() { if (ice()) return 1; return 2; }
int g(int x) { if (ice() && x) return 1; return 2; }
constexpr int h() { return ice() ? 10 : 20; }
constexpr int k(int n) { int s = 0; for (int i = 0; i < n; ++i) s += nice() ? 100 : 1; return s; }
int main() {
  int y = ice();
  int r = (f() == 2 && g(1) == 2 && h() == 20 && k(3) == 3 && y == 0) ? 0 : 1;
  return r ? f() * 100 + g(1) * 10 + h() / 10 : 0;
}
