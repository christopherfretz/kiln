// [expr.const]/20, [meta.const.eval]: `__builtin_is_constant_evaluated()`
// is true in a manifestly constant-evaluated context (static_assert, a
// constexpr variable, an array bound, an enumerator, a case label,
// `if constexpr`). Calls below the top-level expression do not warn. The
// false answer (run time and trial folds) is checked by the codegen
// lowering test `is_constant_evaluated` (crates/kiln_codegen/tests/lower.rs).
//@ expect: compile-ok
//@ strict-diags
namespace std {
constexpr bool is_constant_evaluated() noexcept { return __builtin_is_constant_evaluated(); }
} // namespace std

constexpr int f() { return std::is_constant_evaluated() ? 1 : 2; }
constexpr int g(int n) { return __builtin_is_constant_evaluated() ? n : -n; }
constexpr int h(int n) { return g(n) * 10 + f(); }

static_assert(f() == 1);
static_assert(g(4) == 4);
static_assert(h(2) == 21);
constexpr int a = g(3);
static_assert(a == 3);
int arr[f() + 1];
static_assert(sizeof(arr) == 2 * sizeof(int));
enum E { e = h(1) };
static_assert(e == 11);

constexpr int k(int n) {
  switch (n) {
  case f():
    return 10;
  default:
    return 20;
  }
}
static_assert(k(1) == 10);

int run(int n) {
  if constexpr (f() == 1)
    return n;
  else
    return -n;
}
