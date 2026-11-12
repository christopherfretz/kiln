// [dcl.pre]/10, [dcl.typedef]/2, [module.interface], spec/modules.md
// §11.11 (WP-1320): an imported module's block-scope static_assert
// declarations, typedefs and alias declarations; under `--bmi-image` they
// come from the declaration image and the program behaves exactly as with
// the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imglcd=Inputs/img-local-decls.cppm
import imglcd;
static_assert(lcd::sizes<int>() == 2 * int(sizeof(int)));
static_assert(lcd::nothrow_odr<int>);
int main() {
  int r = lcd::sizes<long>() + lcd::sizes<char>() + lcd::plain();
  return r == 2 * int(sizeof(long)) + 5 ? 0 : 1;
}
