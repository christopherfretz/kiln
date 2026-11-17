// GNU labels as values: `static void *p = &&l;` in a constructor of a
// class with a virtual base, defined in its class, with both the complete
// and the base variant emitted (a derived class's constructor uses the
// base one). The one static names the complete variant's block whatever
// the emission order (spec/codegen.md "Labels as values"). clang emits
// the base variant last here and its static names that variant's block,
// so constructing a `C` jumps into another function: skipped for clang.
// Running the base variant (constructing a `D`) would jump into the
// complete one: not tested (GCC emits one body for both). Found running
// GCC's C++ testsuite.
//@ std: c++20
//@ expect: run
//@ skip(clang): clang's static names the base variant's block, and the complete one crashes
//@ stdout: 2
#include <cstdio>
int hits;
struct A { A() {} };
struct C : virtual A {
  C() {
    static void *p = &&l;
    goto *p;
  l:
    hits += 1;
  }
};
struct D : C { D(); };
D::D() {}
int main() {
  { C c; }
  { C c; }
  std::printf("%d\n", hits);
}
