// [expr.cast]/4: the static_cast a C-style cast performs may convert a pointer or reference to
// derived to one to an inaccessible (private or protected) base, and back, with the base
// adjustment of a static_cast (oneTBB's governor casts a client reference to its privately
// derived dispatcher). C is a non-first base, so the casts must adjust the pointer.
//@ expect: run
//@ stdout: 2 3 2 4 1
#include <cstdio>
struct B { int b = 1; virtual ~B() {} };
struct C { int c = 2; };
struct D : B, private C { int d = 3; };
class E : B, protected C { public: int e = 4; };
int main() {
  D dd;
  E ee;
  const C& c = dd.d ? (const C&)dd : (const C&)ee;  // up to a private base
  const D& d = (const D&)c;                          // down from a private base
  C* pc = (C*)&ee;                                   // protected base
  E* pe = (E*)pc;
  std::printf("%d %d %d %d %d\n", c.c, d.d, pc->c, pe->e, (void*)pc != (void*)&ee);
}
