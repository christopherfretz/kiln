// [basic.def.odr]/15, [namespace.def]/2, spec/modules.md §11.11 "Part 2a":
// two modules that both extend namespace `geo`, and the importer reopens
// it: the importer sees one namespace holding all three. (Under
// `--bmi-image` the first import builds from its image; the second, whose
// names the importer has already seen, replays its source instead.)
//@ expect: run
//@ std: c++20
//@ module: img=Inputs/img.cppm
//@ module: img2=Inputs/img2.cppm
//@ stdout: 3 6 4 2
import img;
import img2;
namespace geo {
int local(int x) { return x * 2; }
}
extern "C" int printf(const char *, ...);
int main() {
  printf("%d %d %d %d\n", geo::add(1, 2), geo::mul(2, 3), geo::local(2), img2_only(3));
}
