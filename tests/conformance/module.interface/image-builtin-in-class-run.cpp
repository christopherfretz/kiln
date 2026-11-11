// [class.mem]/1, [module.interface], spec/modules.md §11.11 (WP-1310): a
// builtin function first used inside an exported class definition is the
// global builtin; under `--bmi-image` the declaration image carries the
// class with the builtin's record among its members and the program
// behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgbic=Inputs/img-builtin-in-class.cppm
import imgbic;

int main() {
  BuiltinUser u{5};
  if (u.f(-3) != 3 || u.g != 5)
    return 1;
  if (!(BuiltinUser::inf() > 1e308))
    return 2;
  return 0;
}
