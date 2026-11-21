//@ expect: run
//@ stdout: 1 5
//@ stdout: 5 5
//@ stdout: 3 0 8
// [expr.const.init]/2 (C++26, P2686R5), [expr.const]: by the text a
// reference to an automatic object of the same function is a permitted
// result, so a non-`constexpr` local reference whose initializer is such a
// constant expression is constant-initialized, usable in constant
// expressions, and its initializer manifestly constant-evaluated: each
// `pick` below would take its `is_constant_evaluated()` / `if consteval`
// branch and bind the local. Deliberate deviation (WP-1830, spec/sema.md
// "Constexpr-referenceable automatic objects", docs/status/deviations.md):
// kiln keeps clang's and GCC's behaviour, the initializer is not constant
// and the run-time branch is taken. The `-gcc-run` twin compares with GCC.
extern "C" int printf(const char *, ...);
namespace std {
constexpr bool is_constant_evaluated() noexcept { return __builtin_is_constant_evaluated(); }
}
int other = 0;
struct P { int a, b; };
P other_p{0, 0};
constexpr int &pick(int &x) {
  if (std::is_constant_evaluated())
    return x;
  return other;
}
constexpr const int &pick_consteval(const int &x) {
  if consteval {
    return x;
  } else {
    return other;
  }
}
constexpr P &pick_pair(P &p) {
  if (std::is_constant_evaluated())
    return p;
  return other_p;
}
int main() {
  int x = 1;
  int &r = pick(x);
  r = 5;
  printf("%d %d\n", x, other);
  int y = 2;
  const int &cr = pick_consteval(y);
  y = 7;
  printf("%d %d\n", cr, other);
  P l{3, 4};
  auto &[a, b] = pick_pair(l);
  a = 8;
  printf("%d %d %d\n", l.a, b, other_p.a);
}
