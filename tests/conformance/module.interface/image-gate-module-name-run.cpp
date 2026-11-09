// [module.interface]/1, [module.import]/1, [namespace.def]/2,
// spec/modules.md §11.11 "Part 2a", "ODR merging" (WP-1096): a module
// that declares a namespace named like itself. `import imgm;` names the
// module without declaring anything, so under `--bmi-image` the unit is
// built from its image (the image gate ignores names interned only as
// module names); the program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgm=Inputs/img-gate.cppm
//@ stdout: 720 9 1 2 42 25
import imgm;
extern "C" int printf(const char *, ...);
static_assert(imgm::fact(5) == 120 && imgm::sq(7) == 49 && imgm::kSeed == 25);
int main() {
  int a = imgm::next();
  int b = imgm::next();
  printf("%d %d %d %d %d %d\n", imgm::fact(6), imgm::sq(3), a, b, imgm::twice(21), imgm::kSeed);
}
