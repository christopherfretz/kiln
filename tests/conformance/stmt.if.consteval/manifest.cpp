// [stmt.if]/4, [expr.const]/21: in every manifestly constant-evaluated
// context `if consteval` takes its compound statement (`if !consteval` its
// else): static_assert, a constexpr variable (namespace scope or local), a
// constinit variable, a template argument, an array bound, an enumerator,
// a case label, an `if constexpr` condition, an immediate invocation
// (WP-588).
//@ expect: compile-ok
//@ strict-diags
constexpr bool ice() { if consteval { return true; } else { return false; } }
constexpr bool nice() { if !consteval { return false; } else { return true; } }
constexpr int h() { return ice() ? 10 : 20; }
constexpr int k(int n) { int s = 0; for (int i = 0; i < n; ++i) s += nice() ? 100 : 1; return s; }
consteval bool cw() { return ice() && nice(); }
consteval int ch() { return h(); }

static_assert(h() == 10);
static_assert(ice() && nice());
static_assert(k(3) == 300);
constexpr bool b = ice();
static_assert(b);
static_assert(cw() && ch() == 10);
constinit bool ci = ice();
template <bool B> struct T { static constexpr bool v = B; };
static_assert(T<ice()>::v);
int arr[h()];
static_assert(sizeof(arr) == 10 * sizeof(int));
enum E { e = h() };
static_assert(e == 10);

int sw(int n) {
  switch (n) {
  case h():
    return 1;
  default:
    return 0;
  }
}

int main() {
  constexpr bool lb = ice();
  static_assert(lb);
  constexpr int lh = h() + k(1);
  static_assert(lh == 110);
  bool r = cw();
  if constexpr (ice()) {
    return (ci && lb && r && sw(10)) ? 0 : 1;
  }
  return 2;
}
