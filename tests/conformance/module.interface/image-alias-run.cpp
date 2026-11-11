// [temp.alias], [dcl.typedef], [module.interface], spec/modules.md §11.11
// "Templates as patterns" (WP-1279): alias templates exported from a
// module, used directly, through an alias and in function template
// parameters the importer deduces; under `--bmi-image` every alias
// template comes from the declaration image and the program behaves
// exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imga=Inputs/img-alias.cppm
//@ stdout: 7 5 9 8 3
import imga;
extern "C" int printf(const char *, ...);
static_assert(sizeof(ia::Arr3<int>) == 3 * sizeof(int));
static_assert(__is_same(ia::Ptr<char>, char *) && __is_same(ia::First<int, char>, int));
int main() {
  int x = 7;
  ia::IntBox b{5};
  ia::BoxOf<long> c{9};
  printf("%d %d %ld %d %d\n", ia::deref(&x), b.get(), ia::unbox(c),
         ia::apply(ia::twice, 4), (int)sizeof(ia::Arr3<char>));
  return 0;
}
