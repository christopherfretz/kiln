// [class.base.init], [temp.class], [module.interface], spec/modules.md
// §11.11 "Class template patterns": a class template's constructors
// with member, base and delegating mem-initializers, instantiated by an
// importer. The declaration image does not carry a pattern's
// mem-initializers, so under `--bmi-image` the module is replayed
// (it once lost the initializers: a miscompile, WP-1139).
//@ expect: run
//@ std: c++20
//@ module: imgcmi=Inputs/img-class-meminit.cppm
//@ stdout: 3 4 5 6 14 15
import imgcmi;
extern "C" int printf(const char *, ...);

int main() {
  Cell<int> a(3), b;
  Sub<long> s(7);
  printf("%d %d %d %d %ld %ld\n", a.x, a.y, b.x, b.y, s.x, s.y);
  return 0;
}
