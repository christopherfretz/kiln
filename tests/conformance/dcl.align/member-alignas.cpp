// [dcl.align]/1, /5, [expr.alignof]: alignof of an expression naming a
// variable or data member is that declaration's alignment (alignas on it
// counts; a member's is capped by what its offset guarantees, as clang's
// getDeclAlign); an alignas pack expansion on a member applies the
// strictest element ([temp.variadic]/5), and alignof of a member whose
// alignment is dependent is value-dependent ([temp.dep.constexpr]/2).
//@ expect: compile-ok
//@ args: -Wno-gnu-alignof-expression
#include <stddef.h>
struct A { char c; alignas(8) char m; char n; int i; };
A a;
static_assert(alignof(a.m) == 8 && alignof(a.n) == 1 && alignof(a.i) == 4);
static_assert(alignof(A::m) == 8);
static_assert(offsetof(A, m) == 8 && sizeof(A) == 16);
template <class... Ts> struct P {
  char c;
  alignas(Ts...) char x;
};
using PSD = P<short, double>;
PSD p;
static_assert(offsetof(PSD, x) == 8 && alignof(p.x) == 8);
static_assert(alignof(PSD) == 8 && sizeof(PSD) == 16);
template <class... Ts> int f(P<Ts...> &v) {
  static_assert(alignof(v.x) == 8);
  return 0;
}
int u = f(p);
struct alignas(16) Q { char a; alignas(16) char b; };
Q q;
// The member's offset (16) and the class's alignment (16) allow 16.
static_assert(alignof(q.b) == 16 && sizeof(Q) == 32);
struct __attribute__((packed)) R { char a; int i; };
R r;
static_assert(alignof(r.i) == 1);
