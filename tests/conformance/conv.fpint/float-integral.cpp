// [conv.fpint] + [conv.double]: floating-to-integral conversion truncates
// toward zero; integral-to-floating is exact when representable.
//@ expect: run
//@ stdout: 2 -2 7 3 -3
//@ stdout: 16777216.0 0.25 3.0
extern "C" int printf(const char*, ...);

int main() {
  double a = 2.9, b = -2.9;
  float f = 7.99f;
  long double ld = 3.75L;
  double nd = -3.5;
  int ia = a, ib = b, iff = f;
  long il = ld;
  short sh = nd;
  printf("%d %d %d %ld %d\n", ia, ib, iff, il, sh);
  int big = 16777216;
  float fb = big;
  float q = 1.0f / 4;
  double d3 = 3;
  printf("%.1f %.2f %.1f\n", fb, q, d3);
}
