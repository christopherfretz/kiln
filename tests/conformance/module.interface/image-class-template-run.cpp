// [temp.class], [temp.mem.func], [temp.static], [temp.mem.class],
// [temp.param]/9, [temp.inst], [module.interface], spec/modules.md
// §11.11 "Class template patterns" (stage S4): class templates exported
// from a module, instantiated by the importer (members defined in the
// class and out of line, static data members, nested types, default
// template arguments, a dependent base, a non-type parameter), in
// constant evaluation and at run time; under `--bmi-image` every
// template is rebuilt from the declaration image and instantiated from
// its pattern, and the program behaves exactly as with the replayed
// source.
//@ expect: run
//@ std: c++20
//@ module: imgct=Inputs/img-class-template.cppm
//@ stdout: 8 7 4 3 9 9 1 6 12 6766 6765 | 8 2.5 4
import imgct;
extern "C" int printf(const char *, ...);
int T = 4, N = 1; // the templates' parameter names: not gated
static_assert(ct::use() == 8 && ct::Box<int>::tag == 7 && ct::Fixed<3>::value == 3);
static_assert(ct::boxed(5L).twice() == 10 && sizeof(ct::Box<char>::value_type) == 1);
static_assert(ct::unbox(ct::Box<short>{3}) == 6);
static_assert(ct::Def<char>{}.sz() == sizeof(ct::W<char>) + sizeof(int));

int main() {
  ct::Box<int> b{4};
  ct::Arr<short> a{9};
  ct::Arr<short>::It it = a.self()->mk();
  ct::Derived<int> d;
  d.x = 1;
  d.y = 3;
  printf("%d %d %d %d %d %d %d %d %d ", b.twice(), ct::Box<long>::tag, b.get(), it.get(),
         (int)it.t, ct::Arr<short>::count, d.bx(), d.dy(), ct::Fixed<6>().f());
  printf("%d %d | ", ct::Heavy<int>{}.get(1), ct::heavy_one() - 1);
  ct::Def<double, float> df{1.0, 2.5f, 4};
  printf("%d %g %d\n", ct::use(), (double)df.b, df.c2 + a.size() - 2);
  return 0;
}
