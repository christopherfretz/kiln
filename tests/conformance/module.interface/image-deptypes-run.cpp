// [temp.res.general]/5, [temp.dep.type], [temp.dep.expr], [dcl.mptr],
// [meta.trans], [over.match.class.deduct], [temp.deduct.guide],
// [module.interface], spec/modules.md §11.11 "Dependent types"
// (WP-1284): dependent names (`typename T::x`, `T::inner::x`, `T::v`),
// type-trait transforms, member pointers, vectors and deduction guides
// (explicit, and implicit ones formed again by the importer) exported
// from a module; under `--bmi-image` they come from the declaration
// image and the program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgd=Inputs/img-deptypes.cppm
//@ stdout: 0 1 5 3 4 5 6 1 7 5 | 7 1 3 8 1 5
import imgd;
extern "C" int printf(const char *, ...);
struct A {
  static constexpr bool value = true;
  struct inner { static constexpr int size = 2; };
  static constexpr int count = 5;
};
static_assert(id::val<A>() && id::sz<A>() == 7);
int main() {
  id::box<int> b;
  const int ci = 5;
  id::no_ref<int &> x = 3;
  id::S s{4};
  printf("%d %d %d %d %d %d %d %d %d %d | ", id::get(b), id::ptr(b) == nullptr, id::rcv(ci), x,
         id::via(s, &id::S::m), id::call(s, &id::S::f), id::vsum(3), id::val<A>(), id::sz<A>(),
         *id::addr<A>());
  int i = 7;
  id::Wrap w(&i);
  id::Wrap c("x");
  id::Wrap d(3);
  id::Agg a{1, 2};
  printf("%d %d %d %d %d %d\n", w.v, c.v, d.v, (int)sizeof(a.a), id::deduce('c'), id::ctad());
  return 0;
}
