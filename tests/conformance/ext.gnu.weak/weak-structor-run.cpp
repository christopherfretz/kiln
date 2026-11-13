// GCC "Common Function Attributes", weak: a weak definition is emitted as
// a weak symbol. For a constructor or destructor that is every variant it
// stands for ([class.ctor], [class.dtor]; the Itanium ABI's complete- and
// base-object constructors C1/C2 and complete-, base-object and deleting
// destructors D1/D2/D0), the complete-object one included when it is an
// alias of the base-object one (clang `getCodegenToUse` gives a weak
// structor the `C5`/`D5` comdat and a `weak` alias). The other unit
// defines the same structors strong, so the program links (no multiple
// definition of a C1 or D1) and the strong ones run; K's weak structors,
// which nothing overrides, are used. A weak member of a class template
// (an implicit instantiation, otherwise `linkonce_odr`) is weak too and
// keeps its C1 alias, which the other unit overrides alone. A downstream
// source file defines `__attribute__((weak))
// Flag::Flag(...)` and is linked
// --whole-archive beside a GCC-built copy of itself.
//@ expect: run
//@ args: %{test_dir}/Inputs/weak-structor-strong.cpp
//@ stdout: strong A
//@ stdout: strong ~A
//@ stdout: strong ~V
//@ stdout: strong ~V
//@ stdout: strong VB
//@ stdout: weak K
//@ stdout: weak ~K
//@ stdout: strong TO C1
#include "Inputs/weak-structor.h"

__attribute__((weak)) A::A(int v) : x(v) { printf("weak A\n"); }
__attribute__((weak)) A::~A() { printf("weak ~A\n"); }
__attribute__((weak)) V::~V() { printf("weak ~V\n"); }
__attribute__((weak)) VB::VB() { printf("weak VB\n"); }
__attribute__((weak)) K::K() { printf("weak K\n"); }
__attribute__((weak)) K::~K() { printf("weak ~K\n"); }
template <class T> __attribute__((weak)) TO<T>::TO(T) { printf("weak TO\n"); }

int main() {
  { A a(1); }      // C1, D1
  delete new V;    // D0
  { V v; }         // D1
  { VB vb; }       // C1 of a class with a virtual base (its own function)
  { K k; }
  { TO<int> t(1); }
}
