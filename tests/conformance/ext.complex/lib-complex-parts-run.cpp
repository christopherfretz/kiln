//@ expect: run
//@ stdout: 1.5 -2.5 1.5 2.5 0.25 0.75 0.25 -0.75
//@ stdout: 1 -0 3 4
// C 7.3.9 (creal, cimag, conj; GNU _Complex in C++): the library functions
// creal / crealf, cimag / cimagf and conj / conjf are builtins that clang
// lowers inline (clang EmitBuiltinExpr, WP-1073): the real part, the
// imaginary part, and the value with its imaginary part negated (so the
// conjugate of +0i has a -0 imaginary part).
#include <cstdio>
extern "C" {
double creal(_Complex double);
float crealf(_Complex float);
double cimag(_Complex double);
float cimagf(_Complex float);
_Complex double conj(_Complex double);
_Complex float conjf(_Complex float);
}

int main() {
  volatile double re = 1.5, im = -2.5;
  _Complex double z = re + im * 1.0i;
  _Complex double cz = conj(z);
  volatile float fr = 0.25f, fi = 0.75f;
  _Complex float w = fr + fi * 1.0if;
  _Complex float cw = conjf(w);
  std::printf("%g %g %g %g %g %g %g %g\n", creal(z), cimag(z), creal(cz), cimag(cz),
              (double)crealf(w), (double)cimagf(w), (double)crealf(cw), (double)cimagf(cw));
  _Complex double one = 1.0;
  _Complex double c1 = conj(one);
  _Complex double k = conj(3.0 - 4.0i);
  std::printf("%g %g %g %g\n", creal(c1), cimag(c1), creal(k), cimag(k));
}
