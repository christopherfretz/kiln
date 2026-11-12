// [dcl.init.general]/16, [temp.res.general], [module.interface],
// spec/modules.md §11.11 (WP-1320): a variable of a dependent type
// directly initialized in an imported class template's member is
// initialized at each instantiation; under `--bmi-image` the initializer
// comes from the declaration image as written and the program behaves
// exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgpi=Inputs/img-paren-init.cppm
import imgpi;
int main() {
  pi::pos<long> a(2);
  pi::pos<long> b = a + 3;
  if (b.off != 5 || a.off != 2)
    return 1;
  return pi::pos<int>::make(7.5).off == 7 ? 0 : 2;
}
