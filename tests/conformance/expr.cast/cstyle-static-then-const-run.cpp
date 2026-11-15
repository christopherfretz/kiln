// [expr.cast]/4.3: a C-style or functional cast that the static_cast alone cannot do because it
// casts away cv-qualification is a static_cast followed by a const_cast, not a reinterpret_cast.
// Converting a pointer to const (or volatile) derived to a pointer to a base at a non-zero offset
// (the derived class has a vptr, the base none) must add the base offset; the downcast subtracts
// it; a pointer to member converts along the hierarchy. Found running GCC's C++ testsuite.
//@ expect: run
//@ stdout: ok
#include <cstdio>
struct A { int a = 1; };
struct B { int b = 2; };
struct D : A, B { virtual ~D() {} int d = 3; };
class P : A { public: virtual ~P() {} int p = 4; };  // A is a private base
using AP = A*;
using BP = B*;
int fails = 0;
void check(bool ok, int line) {
  if (!ok) { std::printf("line %d\n", line); ++fails; }
}
#define CHECK(x) check(x, __LINE__)
int main() {
  D d;
  const D* cd = &d;
  const volatile D* cvd = &d;
  A* a = &d;
  B* b = &d;
  CHECK((A*)cd == a);
  CHECK((B*)cd == b);
  CHECK((B*)cvd == b);
  CHECK((volatile B*)cd == b);
  CHECK(AP(cd) == a);
  CHECK(BP(cvd) == b);
  CHECK(((B*)cd)->b == 2);
  CHECK(&(B&)*cd == b);
  CHECK(&(B&)*cvd == b);
  // Base to derived subtracts the offset.
  const B* cb = b;
  CHECK((D*)cb == &d);
  CHECK((D*)(const volatile B*)b == &d);
  CHECK(&(D&)*cb == &d);
  // An inaccessible base: the cast ignores access ([expr.cast]/4.6, /4.8).
  P p;
  const P* cp = &p;
  A* pa = (A*)cp;
  CHECK(pa->a == 1 && (P*)(const A*)pa == &p);
  // Pointers to members, both directions, cv-qualification cast away.
  const int B::* mb = &B::b;
  int D::* md = (int D::*)mb;
  CHECK(d.*md == 2);
  const int D::* cmd = &D::b;
  int B::* mb2 = (int B::*)cmd;
  CHECK(b->*mb2 == 2);
  if (fails == 0) std::puts("ok");
  return fails;
}
