// [dcl.attr.assume]/1, [expr.const]/5.8 (WP-160): an assumption that is
// evaluated and false makes the expression non-constant (clang's "assumption
// evaluated to false" note at the operator). One with possible side effects is
// not evaluated, only diagnosed with -Wassume; an unreached one is not
// evaluated either.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:15 error "constexpr variable 'bad' must be initialized by a constant expression"
//@ diag: 18:14 note "assumption evaluated to false"
//@ diag: 22:21 note "in call to 'c(-1)'"
//@ diag: 25:12 warning "assumption is ignored because it contains (potential) side-effects [-Wassume]"
//@ diag: 26:12 warning "assumption is ignored because it contains (potential) side-effects [-Wassume]"
//@ diag: 43:16 error "constexpr variable 'bad2' must be initialized by a constant expression"
//@ diag: 39:14 note "assumption evaluated to false"
//@ diag: 43:23 note "in call to 'h<long>(0)'"
int g(int);
constexpr int c(int x) {
  [[assume(x > 0)]];
  return x;
}
static_assert(c(3) == 3);
constexpr int bad = c(-1);
// An assumption with possible side effects is ignored, even when false.
constexpr int d(int x) {
  [[assume(g(x) > 0)]];
  [[assume(++x > 100)]];
  return x;
}
static_assert(d(1) == 1);
// An assumption is not evaluated if it is ill-formed or unreached.
constexpr int e(int x) {
  if (x > 0)
    return x;
  [[assume(x > 0)]];
  return 0;
}
static_assert(e(5) == 5);
template <class T> constexpr T h(T v) {
  [[assume(v != T(0))]];
  return v;
}
static_assert(h(2L) == 2L);
constexpr long bad2 = h(0L);
