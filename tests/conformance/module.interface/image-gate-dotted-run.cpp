// [module.unit]/1, [module.import]/1, [namespace.def]/2,
// spec/modules.md §11.11 "Part 2a", "ODR merging" (WP-1096): the dotted
// module `imga.gate` declares namespaces `imga` and `imga::gate`, named
// like the components of its module name. Neither component declares
// anything in the importer, so under `--bmi-image` the unit is built from
// its image.
//@ expect: run
//@ std: c++20
//@ module: imga.gate=Inputs/img-gate-dotted.cppm
//@ stdout: 15 125 310 307 22
import imga.gate;
extern "C" int printf(const char *, ...);
static_assert(imga::gate::tri(4) == 10 && imga::gate::cube(3) == 27);
int main() {
  printf("%d %d %d %d %d\n", imga::gate::tri(5), imga::gate::cube(5), imga::gate::mix(3),
         imga::gate::mix(3, 7), imga::outer(6));
}
