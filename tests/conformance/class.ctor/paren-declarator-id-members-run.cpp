// [class.ctor.general]/1, [dcl.ambig.res]/1, [dcl.decl.general]/4: after
// the class name and `(`, a pointer, reference or pointer-to-member
// declarator, or a parenthesized name followed by a function suffix, is a
// declarator of a member whose type names the class: `S (*p)(int)`,
// `S (&r)(int)`, `S (S::*pm)(int)`, `S (f)(int)` (a member function),
// `static S (sf)(int)`, `virtual S (vf)(int)`; while `S(int)`,
// `S(S const &)`, `S(int (x), int)`, `explicit S(T, T, T)`, `S(...)`,
// `S([[maybe_unused]] long)`, `S(double) try` and `S() = default` stay
// constructors.
//@ expect: run
//@ stdout: 1 2 3 4 5 6 7 8
extern "C" int printf(const char *, ...);
using T = int;
struct S;
S mk(int);
struct S {
  int v = 0;
  S() = default;
  S(int x) : v(x) {}
  S(S const &) = default;
  S(int (x), int) : v(x + 1) {}
  explicit S(T, T, T) : v(3) {}
  S(...) : v(9) {}
  S([[maybe_unused]] long) : v(4) {}
  S(double) try : v(5) {} catch (...) {}
  S (*p)(int);
  S (&r)(int) = mk;
  S (S::*pm)(int);
  S (f)(int);
  static S (sf)(int);
  virtual S (vf)(int);
  S(S (*q)(int), int) : p(q), pm(&S::f) {}
};
S mk(int x) { return S(x); }
S S::f(int x) { return S(x + 5); }
S S::sf(int x) { return S(x + 6); }
S S::vf(int x) { return S(x + 7); }
int main() {
  S s(mk, 0);
  printf("%d %d %d %d %d %d %d %d\n", s.p(1).v, S(1, 0).v, S(0, 0, 0).v, S(2L).v,
         S(1.0).v, (s.*s.pm)(1).v, S::sf(1).v, s.vf(1).v);
}
