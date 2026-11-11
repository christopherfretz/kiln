// [temp.deduct.guide]/3, [temp.mem]/1, [module.interface],
// spec/modules.md §11.11 "Deduction guides" (WP-1292): a member class
// template's member deduction guide, exported from a module, deduces the
// importer's class template argument; under `--bmi-image` the module
// falls back to replay ("a member deduction guide") and the program
// behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgmg=Inputs/img-memguide.cppm
//@ stdout: 2 4
import imgmg;
extern "C" int printf(const char *, ...);
int main() {
  char c = 0;
  short h = 0;
  mg::O::I a(&c);
  mg::O::I b(&h);
  printf("%d %d\n", a.v, b.v);
  return 0;
}
