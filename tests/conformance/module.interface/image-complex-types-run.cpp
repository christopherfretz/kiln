// [basic.fundamental] (GNU extension: complex types), [module.interface],
// spec/modules.md §11.11 (WP-1320): an imported module's `_Complex` and
// `__complex__` types; under `--bmi-image` they come from the declaration
// image and the program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgcxt=Inputs/img-complex-types.cppm
import imgcxt;
int main() {
  _Complex double d = 3.0;
  __imag__ d = 1.0;
  cxt::box<int> b{cxt::twice(d)};
  cxt::cfloat z = cxt::make(3, 4);
  if (b.real() != 6 || b.imag() != 2)
    return 1;
  return cxt::norm(z) == 25.0f ? 0 : 2;
}
