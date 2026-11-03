// [dcl.attr.noreturn]/1: clang 22 infers noreturn for a function whose body
// is a single throw-expression (inferNoReturnAttr; spec/sema.md §7.5.3):
// -Wmissing-noreturn at its name (void or not, members, a lambda, a member
// of a class template pattern, a function template specialization; not a
// virtual function), ahead of the body's other warnings; -Wreturn-type is
// quiet for a body whose last statement calls an inferred function
// declared earlier (not through parentheses, nor ahead of a null
// statement). clang 19: missing-noreturn-inferred-clang19.cpp.
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wall -Wmissing-noreturn
//@ requires: clang>=22
//@ diag: 28:6 warning "function 'i1' could be declared with attribute 'noreturn'"
//@ diag: 29:5 warning "function 'i2' could be declared with attribute 'noreturn'"
//@ diag: 30:5 warning "function 'i3' could be declared with attribute 'noreturn'"
//@ diag: 32:5 warning "function 'i4' could be declared with attribute 'noreturn'"
//@ diag: 34:7 warning "function 'm' could be declared with attribute 'noreturn'"
//@ diag: 35:14 warning "function 's' could be declared with attribute 'noreturn'"
//@ diag: 39:8 warning "function 'o' could be declared with attribute 'noreturn'"
//@ diag: 43:7 warning "function 'r' could be declared with attribute 'noreturn'"
//@ diag: 46:11 warning "function 'operator()' could be declared with attribute 'noreturn'"
//@ diag: 52:42 warning "non-void function does not return a value in all control paths"
//@ diag: 53:42 warning "non-void function does not return a value in all control paths"
//@ diag: 55:21 warning "non-void function does not return a value"
//@ diag: 56:5 warning "function 'later' could be declared with attribute 'noreturn'"
//@ diag: 40:24 warning "function 't1<int>' could be declared with attribute 'noreturn'"
//@ diag: 41:10 note "in instantiation of function template specialization 't1<int>' requested here"
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
