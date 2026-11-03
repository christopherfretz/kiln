// [expr.prim.id.dtor]/2, [basic.types.general]/9: a pointer to member is a
// scalar type, so `p.~T()` names its pseudo-destructor, directly and
// through a template parameter (libc++ `std::function` holding a member
// function pointer, `__func::destroy`); the call has no effect.
//@ expect: run
//@ stdout: 7 3
extern "C" int printf(const char *, ...);
struct A { int f(int x) const { return x + m; } int m; };
using MF = int (A::*)(int) const;
using MD = int A::*;
template <class F> struct H {
  F f;
  void destroy() { f.~F(); }
};
int main() {
  MF p = &A::f, p2 = p;
  MD q = &A::m, q2 = q;
  typedef MF T;
  p2.~T();
  typedef MD U;
  q2.~U();
  H<MF> h{p};
  h.destroy();
  H<MD>{q}.destroy();
  A a{3};
  printf("%d %d\n", (a.*p)(4), a.*q);
}
