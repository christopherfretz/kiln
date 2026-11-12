// [temp.arg.general]/9, [temp.dep.type]/9, [module.interface],
// spec/modules.md §11.11 (WP-1320): an imported class template named
// with dependent arguments that leave a default to fill, also through a
// member type as a base; under `--bmi-image` the specialization comes
// from the declaration image with its defaults and the program behaves
// exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgddt=Inputs/img-dependent-default-tst.cppm
import imgddt;
struct S {};
int main() {
  if (!ddt::is_signed<int>::value || ddt::is_signed<S>::value)
    return 1;
  ddt::holder<long> h;
  return sizeof(h.bytes) == sizeof(long) ? 0 : 2;
}
