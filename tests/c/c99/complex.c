// C17 6.2.5p11, 6.3.1.8, 6.5.5-6.5.6: _Complex types and arithmetic.
//@ expect: run
//@ stdout: 4 2 -5 10
int printf(const char *, ...);
int main(void) {
  double _Complex a = 1.0 + 2.0 * __builtin_complex(0.0, 1.0);
  double _Complex b = 3.0;
  double _Complex s = a + b;
  double _Complex p = a * a * 1.0 + 0;
  _Complex float f = 2.0f;
  f *= a;
  printf("%g %g %g %g\n", __real__ s, __imag__ s, __real__ (a * a) - 2, __imag__ f * 2.5);
  (void)p;
  return 0;
}
