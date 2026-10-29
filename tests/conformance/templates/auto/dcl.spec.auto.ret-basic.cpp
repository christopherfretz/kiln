// [dcl.spec.auto.general]/3, [dcl.type.auto.deduct]/2.2: a deduced return
// type deduces from the returned operand; no operand deduces `void`.
//@ strict-diags
auto one() { return 1; }
auto two(bool b) {
  if (b)
    return 2L;
  return 3L;
}
auto none() {}
auto bare() { return; }
const auto ci() { return 'c'; }
auto *ptr() { static int i; return &i; }
auto &ref() { static int i; return i; }
constexpr auto sq(int x) { return x * x; }
static_assert(__is_same(decltype(one()), int));
static_assert(__is_same(decltype(two(true)), long));
static_assert(__is_same(decltype(none()), void) && __is_same(decltype(bare()), void));
static_assert(__is_same(decltype(ci()), char));
static_assert(__is_same(decltype(ptr()), int *) && __is_same(decltype(ref()), int &));
static_assert(sq(4) == 16);
