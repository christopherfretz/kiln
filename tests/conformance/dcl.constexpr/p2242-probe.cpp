// [dcl.constexpr]/3 (C++23, P2242R3): a constexpr function may contain a
// non-literal variable, a static or thread_local variable, a label and a
// goto, as long as constant evaluation does not reach them.
// WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
struct NonLiteral { NonLiteral(); };
constexpr int f(bool b) {
  if (b) return 1;
  NonLiteral n;
  static int s = 0;
  thread_local int t = 0;
  goto out;
out:
  return s + t;
}
static_assert(f(true) == 1);
