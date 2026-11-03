// [class.spaceship]/1: a member (or base) with only `==` and `<` is
// compared via the synthesized three-way comparison when the return type
// is a specified comparison category type: `a == b ? equal/equivalent :
// a < b ? less : greater`, and for `partial_ordering`
// `... : b < a ? greater : unordered`; each operand is evaluated once.
//@ expect: compile-ok
#include "../cmp.categories/Inputs/compare.h"
struct L {
  int n;
  constexpr bool operator==(const L&) const = default;
  constexpr bool operator<(const L& o) const { return n < o.n; }
};
struct UseL {
  L l;
  constexpr std::weak_ordering operator<=>(const UseL&) const = default;
};
static_assert((UseL{{1}} <=> UseL{{2}}) < 0);
static_assert((UseL{{2}} <=> UseL{{2}}) == 0);
static_assert((UseL{{3}} <=> UseL{{2}}) > 0);
struct P {
  int n;
  constexpr bool operator==(const P& o) const { return n == o.n; }
  constexpr bool operator<(const P& o) const { return n >= 0 && o.n >= 0 && n < o.n; }
};
struct UseP {
  P p;
  constexpr std::partial_ordering operator<=>(const UseP&) const = default;
};
static_assert((UseP{{1}} <=> UseP{{2}}) == std::partial_ordering::less);
static_assert((UseP{{2}} <=> UseP{{2}}) == std::partial_ordering::equivalent);
static_assert((UseP{{3}} <=> UseP{{2}}) == std::partial_ordering::greater);
static_assert((UseP{{-1}} <=> UseP{{2}}) == std::partial_ordering::unordered);
// A base and an array of a synthesized type, then a built-in member.
struct Base {
  int b;
  constexpr bool operator==(const Base&) const = default;
  constexpr bool operator<(const Base& o) const { return b < o.b; }
};
struct UseS : Base {
  L s[2];
  int x;
  constexpr std::strong_ordering operator<=>(const UseS&) const = default;
};
constexpr UseS mk(int b, int s0, int s1, int x) {
  UseS u{};
  u.b = b;
  u.s[0].n = s0;
  u.s[1].n = s1;
  u.x = x;
  return u;
}
static_assert((mk(1, 2, 3, 4) <=> mk(1, 2, 3, 4)) == std::strong_ordering::equal);
static_assert((mk(0, 2, 3, 4) <=> mk(1, 2, 3, 4)) == std::strong_ordering::less);
static_assert((mk(1, 2, 4, 4) <=> mk(1, 2, 3, 4)) == std::strong_ordering::greater);
static_assert((mk(1, 2, 3, 3) <=> mk(1, 2, 3, 4)) == std::strong_ordering::less);
static_assert(mk(1, 2, 3, 3) < mk(1, 2, 3, 4));
static_assert(mk(1, 2, 3, 5) >= mk(1, 2, 3, 4));
// `==` is called once, then `<` once when they differ.
struct Cnt {
  int* c;
  int n;
  constexpr bool operator==(const Cnt& o) const { ++*c; return n == o.n; }
  constexpr bool operator<(const Cnt& o) const { ++*c; return n < o.n; }
};
struct UseC {
  Cnt k;
  constexpr std::weak_ordering operator<=>(const UseC&) const = default;
};
constexpr int calls(int a, int b) {
  int c = 0;
  UseC x{{&c, a}}, y{{&c, b}};
  (void)(x <=> y);
  return c;
}
static_assert(calls(1, 1) == 1);
static_assert(calls(1, 2) == 2);
static_assert(calls(2, 1) == 2);
