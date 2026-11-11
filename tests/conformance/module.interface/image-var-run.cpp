// [temp.pre]/1, [temp.inst]/7, [module.interface], spec/modules.md §11.11
// "Templates as patterns" (WP-1279): variable templates exported from a
// module (constant, `inline`, mutable, with `auto` and packs) and
// instantiated by the importer; under `--bmi-image` every variable
// template comes from the declaration image and the program behaves
// exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgv=Inputs/img-var.cppm
//@ stdout: 1 314 25 0 0 5
import imgv;
extern "C" int printf(const char *, ...);
static_assert(iv::size<int> == sizeof(int) && iv::is_int<int> && !iv::is_int<char>);
static_assert(iv::sq<4> == 16 && iv::count<int, char> == 2 && iv::twice<long> == 4);
static_assert(!iv::same<int, int> && iv::zero<long> == 0);
int main() {
  iv::counter<int> += 4;
  printf("%d %d %d %d %d %d\n", iv::size<char>, (int)(iv::pi<double> * 100), iv::sq<5>,
         iv::count<>, iv::zero<int>, iv::counter<int>);
  return 0;
}
