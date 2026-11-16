// [class.static.data]/3 in C++14 (N4140 9.4.2/3), [basic.def.odr]/4: a
// `constexpr` static data member odr-used in one unit is defined by the
// out-of-class definition in another, which does not use it. That
// definition is a strong external symbol, emitted although unused there,
// and this unit names it without defining a copy of its own (WP-1517;
// before, kiln emitted neither, so the program did not link).
//@ expect: run
//@ std: c++14
//@ unit: Inputs/constexpr-member-def-cxx14-def.cpp
//@ stdout: 3 3
extern "C" int printf(const char *, ...);
struct A { static constexpr int n = 3; };
int other();
int main() {
  const int *p = &A::n;
  printf("%d %d\n", *p, other());
}
