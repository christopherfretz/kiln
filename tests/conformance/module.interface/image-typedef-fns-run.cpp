// [dcl.fct]/13, [dcl.typedef]/1, [class.derived.general]/2,
// [temp.inst]/3, [module.interface], spec/modules.md §11.11 "Forms
// carried by WP-1369": functions declared through a typedef, an alias
// template, decltype or __typeof, and a base and a member class of an
// implicit instantiation, used by an importer; under `--bmi-image` they
// come from the declaration image and the program behaves exactly as
// with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgtf=Inputs/img-typedef-fns.cppm
//@ stdout: 8 9 10 10
import imgtf;
extern "C" int printf(const char *, ...);
static_assert(noexcept(tf::thrice(1)) && !noexcept(tf::twice(1)));
static_assert(noexcept(tf::Box<int>::quad(1)));
int main() {
  tf::Q q;
  tf::Box<long> b;
  tf::holder<int> h;
  printf("%d %d %d %d\n", tf::twice(), tf::thrice(3), q.inc(4) + q.dec(6), tf::use_holder());
  return b.half(8) == 4 && tf::Box<int>::quad(2) == 8 && h.bump() == 1 &&
                 sizeof(h.s) == 4 && tf::dt_fn(1) + tf::to_fn(1) + tf::base_fn(1) == 6
             ? 0
             : 1;
}
