// [basic.lookup.qual.general]/4.2, [expr.prim.id.dtor]/2: on a dependent
// object, `~Q` in `p->S::F::~Q()` is looked up in the prefix `S` (and,
// as GCC does, in `F`) at instantiation; a dependent `NS::X<T>` names the
// object's class there.
//@ expect: run
//@ oracle: gcc
//@ skip(clang): clang 22 looks the member-qualified namespace `NS` up in the object's class only at instantiation ("use of undeclared identifier 'NS'")
//@ stdout: ~S2 ~S2 ~X |
#include <cstdio>
#include <new>
namespace NS {
struct S2 { using Q = S2; ~S2() { std::printf("~S2 "); } };
using Q2 = S2;
template <class T> struct X { ~X() { std::printf("~X "); } };
}
template <class T> void b3(T *p) { p->NS::S2::~Q2(); }
template <class T> void c3(T *p) { p->NS::S2::~Q(); }
template <class T> void x1(NS::X<T> *p) { p->NS::template X<T>::~X(); }
int main() {
  alignas(16) unsigned char b[64];
  b3(new (b) NS::S2);
  c3(new (b) NS::S2);
  x1(new (b) NS::X<int>);
  std::printf("|\n");
}
