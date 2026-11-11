// [temp.explicit]/2, /10, /12, [module.interface], spec/modules.md
// §11.11 "Explicit instantiations" (WP-1307): a module's explicit
// instantiation declarations and definitions, at namespace scope and
// qualified, take effect in an importer; under `--bmi-image` they come
// from the declaration image and the program behaves exactly as with
// the replayed source (the members the module's definitions instantiate
// are not emitted again).
//@ expect: run
//@ std: c++20
//@ module: imgei=Inputs/img-explicit-inst.cppm
//@ stdout: 4 5 400 500 12 97 15 2
import imgei;
extern "C" int printf(const char *, ...);
int main() {
  ei::X<int> a{4};
  ei::X<long> b{5};
  ei::X<int>::In in{6};
  ei::Y<char> y{'a'};
  ei::Z<int> z{};
  z.v = 5;
  printf("%d %d %d %d %d %d %d %d\n", ei::use(), a.get(), a.big(), b.big(), in.twice(), (int)y.get(), z.f(),
         z.g());
  return 0;
}
