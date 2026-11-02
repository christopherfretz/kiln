// [over.match.oper]/8-9: `x <=> y` also considers the reversed candidate
// `operator<=>(y, x)`; when it is selected the expression is
// `0 <=> (y <=> x)`, so the ordering is inverted. libc++
// `nullopt <=> optional`, `t <=> optional`.
//@ expect: run
#include <compare>
struct N {};
struct O { bool v; };
constexpr std::strong_ordering operator<=>(const O& x, N) noexcept { return x.v <=> false; }
static_assert((O{true} <=> N{}) == std::strong_ordering::greater);
static_assert((N{} <=> O{true}) == std::strong_ordering::less);
static_assert((N{} <=> O{false}) == std::strong_ordering::equal);
// The relational operators through the reversed `<=>`: `0 < (y <=> x)`.
static_assert(N{} < O{true});
static_assert(!(N{} > O{true}));

struct P {
  int i;
  constexpr std::partial_ordering operator<=>(int j) const { return i <=> j; }
};
static_assert((1 <=> P{2}) == std::partial_ordering::less);
static_assert((3 <=> P{2}) == std::partial_ordering::greater);

int main() {
  volatile int x = 3;
  return (x <=> P{2}) == std::partial_ordering::greater && (N{} <=> O{true}) < 0 ? 0 : 1;
}
