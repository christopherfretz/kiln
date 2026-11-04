//@ expect: run
//@ args: -O2
//@ stdout: global 1 20
//@ stdout: local 0 1 20 21 2 1
//@ stdout: runtime 0 2 10
// [stmt.if]/4: an `if consteval` takes its first substatement iff it is
// evaluated in a manifestly constant-evaluated context ([expr.const]/20).
// Constant initializers (a namespace-scope or static variable, a `const`
// integral or `constexpr` local), array bounds and template arguments are
// such contexts; a run-time `if` condition, a conditional operand and a
// non-constant initializer are not, even where the compiler folds them,
// so they see the second substatement (kiln once folded them through the
// first; WP-584, fixed by WP-588). At -O2 the folded and run-time paths must agree.
extern "C" int printf(const char *, ...);
constexpr bool is_ce() {
  if consteval {
    return true;
  } else {
    return false;
  }
}
constexpr int not_ce() {
  if !consteval {
    return 10;
  } else {
    return 20;
  }
}
constexpr int deep(int n) { return n == 0 ? (is_ce() ? 1 : 0) + not_ce() : deep(n - 1); }
template <int N> struct K {
  static constexpr int v = N;
};
int gl = is_ce();
const int gc = not_ce();
int g;
[[gnu::noinline]] void guarded(int a) {
  if (is_ce())
    g = a;
  if (not_ce() == 20)
    g += 100;
}
int main() {
  int a = is_ce();
  const int b = is_ce();
  static int s = not_ce();
  constexpr int c = deep(3);
  int arr[is_ce() + 1];
  guarded(7);
  int t = is_ce() ? 1 : 2;
  printf("global %d %d\n", gl, gc);
  printf("local %d %d %d %d %d %d\n", a, b, s, c, (int)(sizeof(arr) / sizeof(int)), K<is_ce()>::v);
  printf("runtime %d %d %d\n", g, t, deep(2));
}
