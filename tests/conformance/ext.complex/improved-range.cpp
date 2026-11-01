//@ expect: run
//@ strict-diags
//@ args: -fcomplex-arithmetic=improved
//@ stdout: product nan nan
//@ stdout: quotient finite finite
//@ stdout: small 1 2
// -fcomplex-arithmetic=improved (WP-237): the algebraic product; Smith's range-reducing quotient avoids the overflow.
// spec/abi.md §8.17; C23 G.5.1 (via [complex.numbers]): an overflowing
// intermediate shows which algorithm ran.
extern "C" int printf(const char *, ...);
using C = _Complex double;
__attribute__((noinline)) C mul(C a, C b) { return a * b; }
__attribute__((noinline)) C div(C a, C b) { return a / b; }
static const char *kind(double x) {
  return x != x ? "nan" : x - x != 0 ? "inf" : "finite";
}
int main() {
  double inf = __builtin_inf(), nan = __builtin_nan("");
  C x = mul(C{inf, nan}, C{1, 1});
  printf("product %s %s\n", kind(__real__ x), kind(__imag__ x));
  C q = div(C{1e300, 1e300}, C{1e300, 1e300});
  printf("quotient %s %s\n", kind(__real__ q), kind(__imag__ q));
  C s = div(C{-3, 4}, C{1, 2});
  printf("small %g %g\n", __real__ s, __imag__ s);
}
