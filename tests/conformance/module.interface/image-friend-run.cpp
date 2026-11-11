// [class.friend]/2, [class.friend]/7, [namespace.memdef]/3,
// [basic.lookup.argdep]/4, [module.interface], spec/modules.md §11.11
// "Friends" (WP-1292): friends of an exported class, hidden friends
// found by argument-dependent lookup (operators included, in constant
// expressions too), a friend redeclaring an earlier function, and
// befriended classes reading private members, are used by the importer;
// under `--bmi-image` they come from the declaration image and the
// program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgfr=Inputs/img-friend.cppm
//@ stdout: 1 0 12 105 6 5 0
import imgfr;
extern "C" int printf(const char *, ...);
static_assert(fr::Ord{1} < fr::Ord{2} && fr::Ord{3} == fr::Ord{3} && fr::Ord{3} != fr::Ord{4});
int main() {
  fr::Box a(5), b(5), c(6);
  printf("%d %d %d %d %d %d %d\n", a == b, a == c, twice(c), fr::peek(a), fr::Helper::get(a),
         fr::Other::get(c), (int)(fr::Ord{1} < fr::Ord{0}));
  return 0;
}
