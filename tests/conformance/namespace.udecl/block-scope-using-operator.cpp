// [namespace.udecl]/1, [over.match.oper]/3.2, [basic.lookup.unqual]/4: a
// block-scope using-declarator may name a non-member operator function; the
// unqualified lookup for an operator expression finds it from its point of
// declaration to the end of the block, hiding outer declarations of the
// operator (argument-dependent lookup still applies). WP-559.
//@ expect: run
//@ strict-diags
namespace a {
struct V { int x; };
}  // namespace a

namespace ops {
constexpr int operator+(a::V l, a::V r) { return l.x + r.x + 100; }
constexpr bool operator==(a::V l, int r) { return l.x == r; }
constexpr int operator-(a::V v) { return -v.x; }
}  // namespace ops

// Found by ordinary unqualified lookup outside the blocks below.
constexpr int operator+(a::V l, a::V r) { return l.x + r.x; }

constexpr int outer() { return a::V{1} + a::V{2}; }
static_assert(outer() == 3);

constexpr int inner() {
  int before = a::V{1} + a::V{2};
  using ops::operator+;
  int after = a::V{1} + a::V{2};
  return before * 1000 + after;
}
static_assert(inner() == 3 * 1000 + 103);

constexpr bool several() {
  using ops::operator==;
  using ops::operator-;
  return a::V{4} == 4 && -a::V{5} == -5 && operator-(a::V{6}) == -6;
}
static_assert(several());

constexpr int nested() {
  int r = 0;
  {
    using ops::operator+;
    r += a::V{1} + a::V{1};
  }
  return r + (a::V{1} + a::V{1});
}
static_assert(nested() == 102 + 2);

template <class T>
constexpr int tmpl(T v) {
  using ops::operator+;
  return v + v;
}
static_assert(tmpl(a::V{3}) == 106);

int main() {
  if (inner() != 3103) return 1;
  if (tmpl(a::V{1}) != 102) return 2;
  return several() ? 0 : 3;
}
