// [temp.spec.partial], [temp.expl.spec], [temp.inst]/7,
// [module.interface], spec/modules.md §11.11 "Templates as patterns"
// (WP-1307): a module's partial and explicit specializations of
// variable templates are matched and used by an importer; under
// `--bmi-image` they come from the declaration image and the program
// behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgvs=Inputs/img-var-spec.cppm
//@ stdout: 7 1 1 2
import imgvs;
extern "C" int printf(const char *, ...);
static_assert(vs::kind<int **> == 2 && vs::same<int, int> && !vs::same<int, long>);
int main() {
  printf("%d %d %d %d\n", vs::kind<char>, vs::kind<long *>, (int)vs::same<char, char>, vs::kind<char *> - 6);
  return 0;
}
