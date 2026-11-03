// The synthesized three-way comparison ([class.spaceship]/1, clang
// `BuildSynthesizedThreeWayComparison`): a defaulted `operator<=>` whose
// return type is a comparison category compares a subobject with no
// `<=>` as `a == b ? equal/equivalent : a < b ? less : [b < a ?] greater
// [: unordered]`, a `PseudoObjectExpr` whose operands are bound once
// (clang `emitPseudoObjectExpr`), its result an lvalue of the category's
// constants that the comparison copies (`static_cast<R>`).
#include "../conformance/cmp.categories/Inputs/compare.h"
struct L {
  int n;
  bool operator==(const L&) const;
  bool operator<(const L&) const;
};
struct W { L l; std::weak_ordering operator<=>(const W&) const = default; };
struct P { int i; L l; std::partial_ordering operator<=>(const P&) const = default; };
struct S : L { L a[2]; std::strong_ordering operator<=>(const S&) const = default; };
std::weak_ordering cw(const W& x, const W& y) { return x <=> y; }
std::partial_ordering cp(const P& x, const P& y) { return x <=> y; }
bool cs(const S& x, const S& y) { return x < y; }
