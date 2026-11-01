// [over.match.viable]/4, [over.ics.rank]: a candidate whose argument and parameter are
// specializations of the same template notes the bad conversion with clang's elided template
// diff ('[...]' for equal arguments, '(default)', '(no argument)', integer type prefixes)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 59:3 error "no matching function for call to 'f'"
//@ diag: 46:6 note "candidate function not viable: no known conversion from 'pr<[...], int>' to 'pr<[...], char>' for 1st argument"
//@ diag: 60:3 error "no matching function for call to 'g'"
//@ diag: 47:6 note "candidate function not viable: no known conversion from 'vec<char>' to 'vec<int>' for 1st argument"
//@ diag: 61:3 error "no matching function for call to 'h'"
//@ diag: 48:6 note "candidate function not viable: no known conversion from 'tr<[...], vec<char>, [...]>' to 'tr<[...], vec<int>, [...]>' for 1st argument"
//@ diag: 62:3 error "no matching function for call to 'n'"
//@ diag: 49:6 note "candidate function not viable: no known conversion from 'pr<[...], int>' to 'pr<[...], char>' for 1st argument"
//@ diag: 63:3 error "no matching function for call to 'p'"
//@ diag: 50:6 note "candidate function not viable: no known conversion from 'ic<4>' to 'ic<3>' for 1st argument"
//@ diag: 64:3 error "no matching function for call to 's'"
//@ diag: 51:6 note "candidate function not viable: no known conversion from 'tv<long, (long) 1>' to 'tv<int, (int) 1>' for 1st argument"
//@ diag: 65:3 error "no matching function for call to 'q'"
//@ diag: 52:6 note "candidate function not viable: no known conversion from 'tu<[...], int, [...]>' to 'tu<[...], char, [...]>' for 1st argument"
//@ diag: 66:3 error "no matching function for call to 'q'"
//@ diag: 52:6 note "candidate function not viable: no known conversion from 'tu<[...], (no argument), (no argument)>' to 'tu<[...], char, long>' for 1st argument"
//@ diag: 67:3 error "no matching function for call to 'u'"
//@ diag: 53:6 note "candidate function not viable: no known conversion from 'tt<template bag>' to 'tt<template box>' for 1st argument"
//@ diag: 68:3 error "no matching function for call to 'w'"
//@ diag: 54:6 note "candidate function not viable: no known conversion from 'q4<[3 * ...], int>' to 'q4<[3 * ...], char>' for 1st argument"
//@ diag: 69:3 error "no matching function for call to 'x'"
//@ diag: 55:6 note "candidate function not viable: no known conversion from 'pr<volatile int, [...]>' to 'pr<const int, [...]>' for 1st argument"
//@ diag: 70:3 error "no matching function for call to 'y'"
//@ diag: 56:6 note "candidate function not viable: no known conversion from 'pr<vec<[...], char>, [...]>' to 'pr<vec<[...], (default) int>, [...]>' for 1st argument"
//@ diag: 71:3 error "no matching function for call to 'z'"
//@ diag: 57:6 note "candidate function not viable: no known conversion from 'vec<int>' to 'pr<int, char>' for 1st argument"
//@ diag: 72:3 error "no matching function for call to 'g'"
//@ diag: 47:6 note "candidate function not viable: no known conversion from 'vec<[...], char>' to 'vec<[...], (default) int>' for 1st argument"
template<class A, class B> struct pr {};
template<class T, class U = int> struct vec {};
namespace N { template<class A, class B, class C> struct tr {}; }
using I = int;
template<int V> struct ic {};
template<long V> struct lc {};
template<class T, T V> struct tv {};
template<class... T> struct tu {};
template<template<class> class X> struct tt {};
template<class> struct box {};
template<class> struct bag {};
template<class A, class B, class C, class D> struct q4 {};
void f(pr<int, char>);
void g(vec<int>);
void h(N::tr<int, vec<int>, char>);
void n(pr<I, char>);
void p(ic<3>);
void s(tv<int, 1>);
void q(tu<int, char, long>);
void u(tt<box>);
void w(q4<int, int, int, char>);
void x(pr<const int, char>);
void y(pr<vec<int>, char>);
void z(pr<int, char>);
void test() {
  f(pr<int, int>{});
  g(vec<char>{});
  h(N::tr<int, vec<char>, char>{});
  n(pr<int, int>{});
  p(ic<4>{});
  s(tv<long, 1>{});
  q(tu<int, int, long>{});
  q(tu<int>{});
  u(tt<bag>{});
  w(q4<int, int, int, int>{});
  x(pr<volatile int, char>{});
  y(pr<vec<int, char>, char>{});
  z(vec<int>{});
  g(vec<int, char>{});
}
