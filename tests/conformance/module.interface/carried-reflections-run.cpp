// [meta.reflection], [temp.arg.nontype], [module.interface],
// spec/modules.md §11.11 Part 2 (WP-1047): an importer uses
// `std::meta::info` constants the module evaluated, and specializations
// of `Wrap<info R>` keyed on them. A kind-2 module file carries
// reflections as entity references, type encodings or value encodings,
// and keys the specializations by them, so `Wrap<^^S>` in the importer
// is the module's specialization (the same `&Wrap<^^S>::x`), as with
// kind 1 (`--bmi-kind 1|2`). The template parameter object of
// `PW<PH{3, &g}>` is one object in both units ([temp.param]/8).
//@ expect: run
//@ std: c++26
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ oracle: none
//@ module: cr=Inputs/cr.cppm
//@ stdout: 1 20 9
#include "kiln_meta.h"
import cr;
extern "C" int printf(const char *, ...);

static_assert(rS == (^^S) && rg == (^^g) && rint == (^^int) && rcp == (^^const int *));
static_assert(rnull == info() && rns == (^^ns) && rtm == (^^Tm) && rtmi == (^^Tm<int>));
static_assert(ralias == (^^Alias) && ralias != (^^int) && rfn == (^^fnx) && rmem == (^^B::x));
static_assert(rval == std::meta::reflect_constant(42));
static_assert(robj == std::meta::reflect_object(dobj));
static_assert(rbase == std::meta::bases_of(^^D, std::meta::access_context::unchecked())[0]);
static_assert(rparm == std::meta::parameters_of(^^fnx)[1]);
static_assert(h.i == (^^S) && h.p == &g && Wrap<^^S>::self == (^^S));
static_assert(HW<h>::obj->i == (^^S) && HW<h>::obj == HW<Holder{^^S, &g}>::obj);

int main() {
  int *mine[] = {&Wrap<^^S>::x,     &Wrap<^^int>::x, &Wrap<^^ns>::x,    &Wrap<^^Tm>::x,
                 &Wrap<^^Tm<int>>::x, &Wrap<^^Alias>::x, &Wrap<^^B::x>::x,
                 &Wrap<std::meta::reflect_constant(42)>::x, &Wrap<info{}>::x};
  int same = ah == &HW<h>::x && apo == PW<PH{3, &g}>::obj && apo->p == &g;
  int n = 0;
  for (int i = 0; i < 9; ++i) {
    same &= aw[i] == mine[i];
    for (int j = 0; j < i; ++j)
      n += aw[i] == aw[j];
  }
  printf("%d %d %d\n", same && n == 0, Wrap<^^S>::v, int(sizeof aw / sizeof aw[0]));
  return same ? 0 : 1;
}
