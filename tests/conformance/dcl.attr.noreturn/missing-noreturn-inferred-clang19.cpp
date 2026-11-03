// [dcl.attr.noreturn]/1: clang 19 spelling of missing-noreturn-inferred.cpp:
// no inference; a void function whose body is a throw gets
// -Wmissing-noreturn at its body from the CFG, and every caller's fall-off
// warns -Wreturn-type (spec/sema.md §7.5.3).
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wall -Wmissing-noreturn
//@ requires: clang<22
//@ diag: 18:11 warning "function 'i1' could be declared with attribute 'noreturn'"
//@ diag: 38:18 warning "non-void function does not return a value"
//@ diag: 39:40 warning "non-void function does not return a value in all control paths"
//@ diag: 40:42 warning "non-void function does not return a value in all control paths"
//@ diag: 41:43 warning "non-void function does not return a value in all control paths"
//@ diag: 42:42 warning "non-void function does not return a value in all control paths"
//@ diag: 43:42 warning "non-void function does not return a value in all control paths"
//@ diag: 45:21 warning "non-void function does not return a value"
//@ diag: 47:21 warning "non-void function does not return a value"
void i1() { throw 1; }
int i2() { throw 1; }
int i3() { throw; }
struct E { ~E(); };
int i4() { throw E(); }
struct S {
  int m() { throw 1; }
  static int s() { throw 1; }
  int o();
  virtual int v() { throw 1; }
};
int S::o() { throw 1; }
template <class T> int t1() { throw 1; }
int u1 = t1<int>();
template <class T> struct C {
  int r() { throw 1; }
};
int u2 = C<int>().r();
auto l1 = [](int) -> int { throw 1; };
int u3 = l1(0);
int c1() { i2(); }
int c2(int x) { if (x) return 1; i2(); }
int c3(int x) { if (x) return 1; S::s(); }
int c4(int x) { if (x) return 1; S().m(); }
int c5(int x) { if (x) return 1; (i2()); }
int c6(int x) { if (x) return 1; i2(); ; }
int later();
int c7() { later(); }
int later() { throw 1; }
int c8() { later(); }
