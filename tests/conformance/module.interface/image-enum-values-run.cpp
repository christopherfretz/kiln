// [dcl.enum]/5, [dcl.enum]/8, [module.interface], spec/modules.md
// §11.11 "Encoding" (WP-1307): enumerators of enumerations whose fixed
// underlying type is a typedef or `bool` keep their values and types in
// an importer; under `--bmi-image` they come from the declaration image
// and the program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgev=Inputs/img-enum-values.cppm
//@ stdout: 1 255 -300 7 3 256 1
import imgev;
extern "C" int printf(const char *, ...);
static_assert(sizeof(ev::Arg) == 1 && sizeof(ev::Wide) == sizeof(short));
static_assert(ev::pick<ev::Kind::sized>() == 7 && ev::sum() == 256);
int main() {
  ev::Kind k = ev::Kind::sized;
  printf("%d %d %d %d %d %d %d\n", (int)k, (int)ev::Arg::last, (int)ev::lo, ev::pick<ev::Kind::sized>(),
         ev::pick<ev::Kind::unsized>(), ev::sum(), (int)(ev::Arg::boolean == ev::Arg(1)));
  return 0;
}
