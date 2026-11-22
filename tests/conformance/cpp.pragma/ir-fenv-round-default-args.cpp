// [cfenv.syn], C23 7.6.2, [dcl.fct.default]: a default argument is
// evaluated at each call in the FP state it was written in, not in the
// call's: a namespace-scope function's, a member function's (parsed at
// the end of the class), a constructor's, a lambda's, one added by a
// redeclaration and inherited by later ones ([dcl.fct.default]/4), a
// block-scope declaration's, and an instantiated one (a function
// template's, a class template member's, a member template's), which
// keeps its pattern's state. A caller whose own state is not constrained
// gets plain instructions. Compared with clang's IR exactly
// (spec/codegen.md "Floating-point semantics", WP-1919).
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -Wno-unknown-pragmas
double gx;
#pragma STDC FENV_ROUND FE_UPWARD
double f(double x = gx * 3.0) { return x; }
struct C {
  double m(double y = gx / 3.0) { return y; }
  C(double v = gx - 13.0) : c(v) {}
  double c;
};
template <class T> T tf(T x = gx - 7.0) { return x; }
template <class T> struct TC {
  T m(T x = gx + 9.0) { return x; }
  template <class U> U mt(U x = gx * 21.0) { return x; }
};
auto lam = [](double x = gx / 19.0) { return x; };
double g2(double z);
#pragma STDC FENV_ROUND FE_TOWARDZERO
double g2(double z = gx * 11.0);
double h(double w = f() * 2.0) { return w; }
#pragma clang fp reassociate(on)
double r(double a = gx + 1.0 + gx) { return a; }
#pragma clang fp reassociate(off)
#pragma STDC FENV_ROUND FE_DOWNWARD
double g2(double z);
C gc;
TC<double> gt;
double use() {
  double s = f() + gc.m() + gc.c + tf<double>() + gt.m() + gt.mt<double>();
  s += lam() + g2() + h() + r();
  return s;
}
double use_local() {
#pragma STDC FENV_ROUND FE_TOWARDZERO
  double lf(double = gx * 17.0);
  return lf();
}
double g2(double z) { return z; }
double lf(double q) { return q; }
#pragma STDC FENV_ROUND FE_TONEAREST
double use_near() { return f() + r(); }
