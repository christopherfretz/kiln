// [basic.def.odr]/15, [namespace.def]/2, [basic.scope.namespace],
// spec/modules.md §11.11 "Part 2a", "ODR merging" (WP-1096): the importer
// declares namespace `imgm` and a variable `twice` before `import imgm;`,
// and the module's image declares both names. A real earlier declaration
// still replays the module under `--bmi-image`: the importer's namespace
// and the module's are one namespace, and `::twice` is a different
// entity from `imgm::twice`.
//@ expect: run
//@ std: c++20
//@ module: imgm=Inputs/img-gate.cppm
//@ stdout: 7 24 120 3 1
namespace imgm {
int mine() { return 7; }
} // namespace imgm
int twice = 3;
import imgm;
extern "C" int printf(const char *, ...);
static_assert(imgm::fact(5) == 120);
int main() {
  printf("%d %d %d %d %d\n", imgm::mine(), imgm::twice(12), imgm::fact(5), twice, imgm::next());
}
