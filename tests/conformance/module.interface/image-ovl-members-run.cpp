// [expr.ref]/7, [over.match.call]/2, [over.match.funcs]/4,
// [namespace.udecl]/14, [class.member.lookup], [temp.arg.explicit]/1,
// [module.interface], spec/modules.md §11.11 "Forms carried by WP-1368":
// an imported module's overloaded member accesses in dependent calls
// (implicit `this`, `.` and `->`, a static candidate, explicit template
// arguments, a hidden base member, a using-declaration, a class
// template's `this->f`) choose at each instantiation the overload the
// replayed source does; under `--bmi-image` the candidate sets come from
// the declaration image.
//@ expect: run
//@ std: c++26
//@ module: imgovl=Inputs/img-ovl-members.cppm
import imgovl;

using namespace om;
static_assert(dist_fn{}(vec{}) == 4 && dist_fn{}.twice(vec{}) == 8);
static_assert(dot(1) == 1 && dot(1L) == 2 && dot(1.0) == 3);
static_assert(arrow(1) == 1 && arrow(1L) == 2 && arrow(1.0) == 3);
static_assert(targs(1) == 30 && targs(1L) == 20);
static_assert(hid(1) == 2 && hid("x") == 5);
static_assert(used(1) == 1 && used('c') == 4 && used(1L) == 3);
static_assert(gen<int>{5}.run(2) == 7 && callp(1L) == 1);

int main() {
  vec v{};
  gen<long> g{1};
  if (dist_fn{}(v) + dist_fn{}.twice(v) != 12)
    return 1;
  if (dot(1L) + arrow(1.0) + targs(1) + hid(1) + used(1.5) != 43)
    return 2;
  return g.run(1) == 2 ? 0 : 3;
}
