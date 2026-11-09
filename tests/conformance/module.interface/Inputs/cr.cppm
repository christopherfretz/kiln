// Interface unit of module `cr` for carried-reflections-run.cpp
// (spec/modules.md §11.11 Part 2, WP-1047): constexpr reflections of
// each kind kiln carries, a class holding one, and specializations of
// `Wrap<info R>` keyed on them, all evaluated here.
module;
#include "kiln_meta.h"
export module cr;
export using std::meta::info;
export namespace ns {
int z = 3;
}
export template <class T> struct Tm {
  T t;
};
export using Alias = int;
export int fnx(int a, int b) { return a + b; }
export struct B { int x = 1; };
export struct D : B { int y = 2; };
export D dobj;
export struct S {};
export int g = 4;
export constexpr info rS = ^^S;
export constexpr info rg = ^^g;
export constexpr info rint = ^^int;
export constexpr info rcp = ^^const int *;
export constexpr info rnull = info();
export constexpr info rns = ^^ns;
export constexpr info rtm = ^^Tm;
export constexpr info rtmi = ^^Tm<int>;
export constexpr info ralias = ^^Alias;
export constexpr info rfn = ^^fnx;
export constexpr info rmem = ^^B::x;
export constexpr info rval = std::meta::reflect_constant(42);
export constexpr info robj = std::meta::reflect_object(dobj);
export constexpr info rbase = std::meta::bases_of(^^D, std::meta::access_context::unchecked())[0];
export constexpr info rparm = std::meta::parameters_of(^^fnx)[1];
export struct Holder {
  info i;
  int *p;
};
export constexpr Holder h{^^S, &g};
export template <info R> struct Wrap {
  static int x;
  static constexpr info self = R;
  static constexpr int v = sizeof(int) * 5;
};
template <info R> int Wrap<R>::x = 0;
export template <Holder H> struct HW {
  static int x;
  static constexpr const Holder *obj = &H;
};
template <Holder H> int HW<H>::x = 0;
export int *aw[] = {&Wrap<^^S>::x,     &Wrap<^^int>::x, &Wrap<^^ns>::x,    &Wrap<^^Tm>::x,
                    &Wrap<^^Tm<int>>::x, &Wrap<^^Alias>::x, &Wrap<^^B::x>::x,
                    &Wrap<std::meta::reflect_constant(42)>::x, &Wrap<info{}>::x};
export int *ah = &HW<h>::x;
export struct PH {
  int n;
  int *p;
};
export template <PH H> struct PW {
  static constexpr const PH *obj = &H;
};
export const PH *apo = PW<PH{3, &g}>::obj;
static_assert(Wrap<^^S>::self == (^^S) && Wrap<^^S>::v == 20 && HW<h>::obj->i == (^^S));
