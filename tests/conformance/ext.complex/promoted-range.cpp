//@ expect: run
//@ strict-diags
//@ args: -fcomplex-arithmetic=promoted
//@ stdout: product nan nan
//@ stdout: quotient 1 0
//@ stdout: quotientf 1 0
//@ stdout: small -3 4
//@ stdout: compound 1 0
//@ stdout: real 0.5 0.5
// WP-1795: -fcomplex-arithmetic=promoted: the algebraic product (no NaN
// recovery), and a quotient by a complex divisor computed algebraically
// in the next wider type, where the intermediate `cc + dd` of a huge
// divisor does not overflow (spec/codegen.md "Promoted complex
// arithmetic"); C23 G.5.1 (via [complex.numbers]).
extern "C" int printf(const char *, ...);
using C = _Complex double;
using CF = _Complex float;
__attribute__((noinline)) C mul(C a, C b) { return a * b; }
__attribute__((noinline)) C div(C a, C b) { return a / b; }
__attribute__((noinline)) CF divf(CF a, CF b) { return a / b; }
__attribute__((noinline)) void cdiv(C &a, C b) { a /= b; }
__attribute__((noinline)) C rdiv(double a, C b) { return a / b; }
static const char *kind(double x) {
  return x != x ? "nan" : x - x != 0 ? "inf" : "finite";
}
int main() {
  double inf = __builtin_inf(), nan = __builtin_nan("");
  C x = mul(C{inf, nan}, C{1, 1});
  printf("product %s %s\n", kind(__real__ x), kind(__imag__ x));
  C q = div(C{1e300, 1e300}, C{1e300, 1e300});
  printf("quotient %g %g\n", __real__ q, __imag__ q);
  CF qf = divf(CF{1e30f, 1e30f}, CF{1e30f, 1e30f});
  printf("quotientf %g %g\n", (double)__real__ qf, (double)__imag__ qf);
  C s = div(C{5, 10}, C{1, 2});
  C t = mul(s, C{-3, 4});
  printf("small %g %g\n", __real__ t / 5, __imag__ t / 5);
  C a{2e300, 2e300};
  cdiv(a, C{2e300, 2e300});
  printf("compound %g %g\n", __real__ a, __imag__ a);
  C r = rdiv(1, C{1, 1});
  printf("real %g %g\n", __real__ r, -__imag__ r);
}
