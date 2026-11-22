// [cfenv.syn], C23 7.6.2, [class.base.init]: a constructor's
// mem-initializers (member, base and delegating) are evaluated in the FP
// state they were written in, not in the body's: a pragma at the start of
// the body does not reach them, and an out-of-class definition under
// another file-scope `FENV_ROUND` uses that one. A default member
// initializer keeps the state it was written in wherever it is used (an
// implicit or user-written constructor, aggregate initialization, an
// instantiation), as clang keeps the state on each expression. Compared
// with clang 22's IR exactly (spec/codegen.md "Floating-point
// semantics", WP-1904).
//@ expect: ir
//@ requires: clang>=22
//@ args: --target=aarch64-unknown-linux-gnu -Wno-unknown-pragmas
double gx;
#pragma STDC FENV_ROUND FE_UPWARD
struct B {
  double b;
  B(double x) : b(x) {}
};
struct S : B {
  double m;
  double dm = gx * 3.0;
  S(double a, double c) : B(a / c), m(a + c) {
#pragma STDC FENV_ROUND FE_TOWARDZERO
    m = m / a;
  }
  S(double a) : S(a, a - 2) {}
  S(int a);
};
template <class T> struct TT {
  T dm = gx * 5.0;
  T m;
  TT(T a, T b) : m(a - b) {}
  TT(T a);
};
struct Agg {
  double a = gx / 7.0;
  double b;
};
struct D : B {
  double m;
  D(double a) try : B(a * 3.0), m(a / 3.0) {
  } catch (...) {
  }
};
struct Imp {
  double i = gx - 0.5;
};
#pragma STDC FENV_ROUND FE_DOWNWARD
S::S(int a) : B(a * 0.25), m(a) {}
template <class T> TT<T>::TT(T a) : m(a + 1.0) {}
S s1(1.0, 2.0), s2(1.0), s3(4);
TT<double> t1(1.0, 2.0), t2(1.0);
D d(1.0);
Imp imp;
double use_agg(double y) {
  Agg g{.a = gx * 1.5, .b = y * 2.0};
  Agg h{.b = y};
  return g.a + h.a;
}
#pragma STDC FENV_ROUND FE_TONEAREST
struct N {
  double n = gx * 9.0;
  N() {}
  N(int) : n(gx - 1.0) {}
};
N n1, n2(1);
