// [temp.expl.spec]/1, [namespace.def]/2, [module.interface],
// spec/modules.md §11.11 (WP-1314): an imported module's explicit
// specializations declared in one namespace block and defined in a later
// one are carried by the declaration image under `--bmi-image`.
//@ expect: run
//@ std: c++20
//@ module: imgrs=Inputs/img-reopened-spec.cppm
import imgrs;
static_assert(rs::traits<char>::size() == 1);
int main() {
  rs::traits<char>::type c = 2;
  return c - 2;
}
