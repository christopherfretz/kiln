// [meta.reflection], [meta.reflection.define.aggregate],
// [meta.reflection.annotation], [module.interface], spec/modules.md
// §11.11 Part 2 (WP-1160): an importer uses `std::meta::info` constants
// the module evaluated that reflect alias template specializations, data
// member specifications and annotations. A kind-2 module file carries
// them as the alias template with its arguments, the specification's
// contents and the annotation's entity, position and ordinal, so each
// compares equal to the importer's own reflection of the same entity, as
// with kind 1 (`--bmi-kind 1|2`).
//@ expect: run
//@ std: c++26
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ oracle: none
//@ module: cx=Inputs/cx.cppm
//@ stdout: 1 1 3
#include "kiln_meta.h"
import cx;
extern "C" int printf(const char *, ...);
using namespace std::meta;
static_assert(rap == (^^Ptr<int>) && rap != (^^int *));
static_assert(rla == (^^LA<int, char>) && rla != (^^LA<int>) && rla != (^^L<int, char, long>));
static_assert(rapp == (^^App<Tm>) && rapp != (^^Tm<int>));
static_assert(rarr == (^^Arr<char, 3>) && rarr != (^^Arr<char, 4>) && rarr != (^^char[3]));
static_assert(rd1 == data_member_spec(^^int, {.name = "x"}) && rd1 != rd4);
static_assert(rd2 == data_member_spec(^^char, {.name = "y", .alignment = 8}));
static_assert(rd3 == data_member_spec(^^unsigned, {.bit_width = 3}));
static_assert(rd4 == data_member_spec(^^int, {.name = "z", .no_unique_address = true}));
static_assert(ra0 == annotations_of(^^A)[0] && ra1 == annotations_of(^^A)[1]);
static_assert(ra2 == annotations_of(^^A)[2] && ra0 != ra2);
static_assert(ram == annotations_of(^^A::m)[0]);
static_assert(rab == annotations_of(bases_of(^^C, access_context::unchecked())[0])[0]);
static_assert(extract<int>(ra2) == 1 && extract<int>(rab) == 5);
struct T;
consteval { define_aggregate(^^T, {rd1, rd2, rd3, rd4}); }
int main() {
  T t{};
  t.x = 1;
  printf("%d %d %d\n", int(sizeof(App<Tm>)), t.x, extract<int>(ram));
}
