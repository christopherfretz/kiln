// [temp.dep.candidate], [class.member.lookup], [temp.inst]/4: a header's
// unqualified and class-qualified calls of member function templates
// (`pick<T>(t)`, `holder::pick(t)`, `impl::test<T>(0)`) and a dependent
// decltype base resolve at each instantiation. With `--pch-diff`, the
// header's snapshot carries the names with the members they
// found and their naming class (WP-1314).
//@ expect: run
#include "Inputs/member_names.h"
struct G {
  int operator()(int x) const { return x; }
};
int main() {
  int r = mf::holder<int>::go(3) + mf::holder<long>().inst(4L);
  if (r != 18 || !mf::is_ref2<int>::value || mf::is_ref2<void>::value)
    return 1;
  mf::inv<G>::R x = 5;
  return x == 5 ? 0 : 2;
}
