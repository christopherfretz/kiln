//@ expect: run
//@ stdout: 9 18 1
// WP-726: `&C::m` as a template argument inside C's own definition (C
// incomplete; boost.intrusive's `member_hook<C, H, &C::hook>`, folly
// TimeoutManager): the same argument as after C is complete.
#include <cstdio>
#include <type_traits>
template <class C, class H, H C::*P> struct member_hook {
  static H& get(C& c) { return c.*P; }
};
template <class C, int (C::*F)() const> struct fn_hook {
  static int call(const C& c) { return (c.*F)(); }
};
struct Cob {
  int a = 1;
  int hook = 5;
  int twice() const { return 2 * hook; }
  using L = member_hook<Cob, int, &Cob::hook>;
  using F = fn_hook<Cob, &Cob::twice>;
};
static_assert(std::is_same_v<Cob::L, member_hook<Cob, int, &Cob::hook>>);
static_assert(std::is_same_v<Cob::F, fn_hook<Cob, &Cob::twice>>);
static_assert(!std::is_same_v<Cob::L, member_hook<Cob, int, &Cob::a>>);
int main() {
  Cob c;
  Cob::L::get(c) = 9;
  std::printf("%d %d %d\n", c.hook, Cob::F::call(c), member_hook<Cob, int, &Cob::a>::get(c));
}
