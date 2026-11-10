// Interface unit of module `cx` for carried-reflections-more-run.cpp
// (spec/modules.md §11.11 Part 2, WP-1160): constexpr reflections of
// alias template specializations (type, pack and template arguments),
// data member specifications and annotations (of a class, a member and
// a base specifier; two equal ones at different positions), all
// evaluated here, so a kind-2 module file carries them.
module;
#include "kiln_meta.h"
export module cx;
export using std::meta::info;
export template <class T> using Ptr = T *;
export template <class... Ts> struct L {};
export template <class... Ts> using LA = L<Ts..., long>;
export template <class T> struct Tm {};
export template <template <class> class TT> using App = TT<int>;
export constexpr info rap = ^^Ptr<int>;
export constexpr info rla = ^^LA<int, char>;
export constexpr info rapp = ^^App<Tm>;
export constexpr info rd1 = std::meta::data_member_spec(^^int, {.name = "x"});
export constexpr info rd2 = std::meta::data_member_spec(^^char, {.name = "y", .alignment = 8});
export constexpr info rd3 = std::meta::data_member_spec(^^unsigned, {.bit_width = 3});
export constexpr info rd4 = std::meta::data_member_spec(^^int, {.name = "z", .no_unique_address = true});
export struct [[=1, =2, =1]] A {
  [[=3]] int m;
};
export struct C : [[=5]] A {};
export constexpr info ra0 = std::meta::annotations_of(^^A)[0];
export constexpr info ra1 = std::meta::annotations_of(^^A)[1];
export constexpr info ra2 = std::meta::annotations_of(^^A)[2];
export constexpr info ram = std::meta::annotations_of(^^A::m)[0];
export constexpr info rab = std::meta::annotations_of(std::meta::bases_of(^^C, std::meta::access_context::unchecked())[0])[0];
