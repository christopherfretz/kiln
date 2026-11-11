// [temp.inst]/3, [class.bit], [class.mem.general], [class.static.data],
// [module.interface], spec/modules.md "Templates as patterns" (WP-1300):
// an implicit class template specialization the module completed keeps
// its pattern's bit-field widths, default member initializers and
// in-class static data member initializers in the importer. The image
// reader instantiates a complete specialization before the pattern's
// expressions are set, so one whose pattern has a bit-field or a default
// member initializer is not carried (it read `F<3>::a` as a plain `int`,
// a miscompile), and a static data member initializer it could not see
// is instantiated afterwards (`W<int>::w` was undefined).
//@ expect: run
//@ std: c++20
//@ module: imgsl=Inputs/img-spec-late.cppm
//@ stdout: 5 5 3 4 4
import imgsl;
extern "C" int printf(const char *, ...);
int main(int argc, char **) {
  F<3> y{};
  y.a = unsigned(argc) + 8;
  static_assert(W<int>::w == sizeof(int));
  printf("%d %d %d %d %d\n", f(9), int(y.a) + y.b, F<3>{}.b - 1, h(), W<int>::w);
  return 0;
}
