// [temp.variadic], [expr.prim.fold], [expr.sizeof]/5, [temp.param]/10,
// [dcl.spec.auto], [module.interface], spec/modules.md §11.11 "Templates
// as patterns" (WP-1279): variadic templates, fold-expressions,
// `sizeof...`, `auto` in patterns and non-type default template
// arguments exported from a module and instantiated by the importer;
// under `--bmi-image` every template comes from the declaration image and
// the program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgp=Inputs/img-pack.cppm
//@ stdout: 1 3 10 11 1 7 0 6 2 3 9 | 4 4 8 5
import imgp;
extern "C" int printf(const char *, ...);
static_assert(ip::count<int, char, long>() == 3 && ip::sum(1, 2, 3) == 6);
static_assert(ip::nsum<1, 2, 4>() == 7 && ip::inc(1.5) == 2.5);
static_assert(ip::def() == 6 && ip::def<5>() == 10 && ip::sz<char>() == 1);
static_assert(ip::Box<int, int>::n == 2 && ip::Arr<char>{}.len() == 4);
int main() {
  ip::Box<int, char, long> b;
  ip::Arr<int> a{};
  ip::Arr<short, 5> c{};
  printf("%d %d %d %d %d %d %d %d %d %d %d | %d %d %d %d\n", ip::count<int>(),
         ip::count_args(1, 2.0, 'c'), ip::sum(1, 2, 3, 4), ip::lsum(5, 6),
         (int)ip::all(true, 1, 2), ip::sum(3, 4), ip::nsum<>(), ip::def(), ip::sz<short>(),
         b.size(), ip::first(9, 1.0), a.len(), ip::inc(a.v) + 3,
         ip::twice(4), c.len());
  return 0;
}
