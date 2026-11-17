// GNU labels as values: a function-local `static` initialized with label
// addresses (`static void *p = &&l;`, a constant initializer) in a body
// that is emitted more than once. A constructor or destructor of a class
// with a virtual base has distinct complete and base variants (C1/C2,
// D1/D2) sharing the one static; as clang does, the static's initializer
// is emitted again with each variant (the last one emitted, the complete
// variant, owns it), and that variant lists the labels as its computed
// `goto` destinations. kiln named the base variant's blocks, so the
// complete variant jumped into another function (SIGSEGV). Running the
// base variant (a class derived from `C`) would still jump into the
// complete one, as with clang: not tested (GCC emits one body). The
// constructor is defined out of its class here, where clang emits the
// complete variant last (run-static-label-in-class-constructor.cpp has
// the other order). Also: label
// tables in a class template member, a lambda, an inline function, label
// differences, and a label in a `try` block and a range-`for` body. Found
// running GCC's C++ testsuite.
//@ std: c++20
//@ expect: run
//@ stdout: 1 2 3 4 20 10 5 6 100 200 7 9
#include <cstdio>
int hits;
struct A { A() {} virtual ~A() {} };
struct C : virtual A {
  C();
  ~C() {
    static void *const tbl[] = {&&x, &&y};
    goto *tbl[hits & 1];
  x:
    hits += 10;
    return;
  y:
    hits += 1;
  }
};
C::C() {
  static void *p = &&l;
  goto *p;
l:
  hits += 1;
}
int f(int i) {
  static const void *tbl[] = {&&a, &&b, &&c};
  goto *tbl[i];
a: return 1;
b: return 2;
c: return 3;
}
template <class T> struct X {
  int g(int i) {
    static const void *tbl[] = {&&a, &&b};
    goto *tbl[i];
  a: return 10;
  b: return 20;
  }
};
inline int h(int i) {
  static const int off[] = {0, (int)((char *)&&b - (char *)&&a)};
  goto *((char *)&&a + off[i]);
a: return 100;
b: return 200;
}
int lam(int i) {
  auto l = [](int i) {
    static void *t[] = {&&x, &&y};
    goto *t[i];
  x: return 5;
  y: return 6;
  };
  return l(i);
}
int in_try(int i) {
  try {
    static void *t[] = {&&x, &&y};
    goto *t[i];
  x: return 7;
  y: return 8;
  } catch (...) {
  }
  return 0;
}
int in_for(int i) {
  int r = 0;
  int ks[] = {0, 1};
  for (int k : ks) {
    static void *t[] = {&&x, &&y};
    goto *t[(i + k) & 1];
  x: r += 4; continue;
  y: r += 5;
  }
  return r;
}
int main() {
  { C c; }      // complete-object variants: +1 then +1 (odd)
  int a = hits; // 2
  { C c; }
  std::printf("%d %d %d %d %d %d %d %d %d %d %d %d\n", f(0), a, f(2), hits,
              X<int>().g(1), X<int>().g(0), lam(0), lam(1), h(0), h(1),
              in_try(0), in_for(0));
}
