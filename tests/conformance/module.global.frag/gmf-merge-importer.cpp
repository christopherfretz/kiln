// [module.global.frag]/1, [basic.def.odr]/15, [module.reach]/5: the global
// module fragments of a module interface unit and of the unit it imports
// declare and define the same entities (a class, variables, a variable
// template and its specialization, a class template, functions, a function
// template, enumerations — one opaque, used before its definition —
// typedefs, a concept, an alias template), the importer's GMF first. They
// are one entity each: no redefinition, each unit sees its own
// declarations, and the importer of both finds them through either unit's
// exported using-declarations (a scoped enumerator through its reachable
// enumeration). kiln builds gmi_inner in-process after gmi_outer's GMF and
// merges its definitions with the earlier ones (spec/modules.md §6).
//@ expect: compile-ok
//@ std: c++20
//@ module: gmi_inner=Inputs/gmi-inner.cppm
//@ module: gmi_outer=Inputs/gmi-outer.cppm
import gmi_outer;
static_assert(inner() == 8 + 7 + 3 + 9 + 6 + 2 + 2);
static_assert(outer() == inner());
static_assert(n::S{}.get() == 8 && n::g == 7 && n::z<long> == 3 && n::z<char> == 9);
static_assert(n::W<int>{1}.f() == 2 && n::h(1) == 2 && n::ht(3) == 3);
static_assert(n::e1 == 5 && static_cast<int>(n::C::c1) == 1 && n::c_small);
static_assert(sizeof(n::I) == sizeof(int) && n::K<n::J>);
n::A<int> p = nullptr;
int main() {}
