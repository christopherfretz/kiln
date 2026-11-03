// [expr.ref]/1, /6-8 P2996R13: in `E1.[: r :]` and `E1->[: r :]`, r
// designates a member of E1's class or of one of its bases: a non-static
// data member (an lvalue of the object's member), a static data member, a
// member function (called virtually: a splice is not a qualified-id,
// [expr.call]/2), a static member function and an enumerator. Run through
// codegen.
//@ expect: run
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: 10 2 3 4 5 6 7 8 9
#include "kiln_meta.h"
extern "C" int printf(const char*, ...);
struct B {
  int x = 1;
  static int s;
  virtual int f() const { return 0; }
  int g() const { return x + 1; }
  static int h(int a) { return a + 5; }
  enum { Seven = 7 };
};
int B::s = 3;
struct D : B {
  int y = 4;
  int f() const override { return 6; }
};
constexpr std::meta::info rx = ^^B::x;

int main() {
  D d;
  D* pd = &d;
  const B& b = d;
  d.[: rx :] = 10;
  int x = pd->[: ^^B::x :];
  d.[: rx :] = 1;
  printf("%d %d %d %d %d %d %d %d %d\n", x, b.[: ^^B::g :](), d.[: ^^B::s :],
         pd->[: ^^D::y :], b.[: ^^B::h :](0), b.[: ^^B::f :](), d.[: ^^B::Seven :],
         pd->[: ^^D::y :] * 2, d.[: ^^D::y :] + 5);
  return 0;
}
