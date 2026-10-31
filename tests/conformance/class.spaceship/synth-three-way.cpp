// [class.spaceship]/1: a member with only `==` and `<` is compared via the
// synthesized three-way comparison when the return type is a specified
// comparison category type.
//@ expect: compile-ok
//@ xfail(kiln): the synthesized three-way comparison needs a PseudoObjectExpr (spec/sema-class.md §11 Q12); kiln reports it as not supported yet
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
