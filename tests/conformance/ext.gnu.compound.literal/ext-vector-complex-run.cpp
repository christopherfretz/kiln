// Compound literals of ext_vector_type and _Complex type (GNU/clang ext):
// element access on a prvalue vector literal (xyzw, rgba, sN, hi/lo/even/odd),
// swizzle stores through an lvalue, and a two-element complex literal
// initializing the real and imaginary parts ([dcl.init.list] scalar rule
// does not apply to complex).
//@ expect: run
//@ stdout: 2 1 4 3 3 3 4 7 4 8 2 8 8 1 2
extern "C" int printf(const char*, ...);
typedef float f4 __attribute__((ext_vector_type(4)));
typedef float f2 __attribute__((ext_vector_type(2)));
typedef int i4 __attribute__((ext_vector_type(4)));

float lit(float x) { return ((f4){1, 2, 3, 4}).y + ((f4){x, 2, 3, 4}).x; }
int main() {
  f4 v = {1, 2, 3, 4};
  printf("%g %g %g %g ", ((f4){1, 2, 3, 4}).y, v.r, v.a, v.s2);
  f2 s = v.xy + v.hi + v.odd;  // {1,2} + {3,4} + {2,4} = {6,10}
  printf("%g %g ", lit(1.0f), s.x - v.x - v.y);
  i4 w = {1, 2, 3, 4};
  w.y = 9;
  w.xz = (i4){7, 8, 0, 0}.xy;
  w.wy = w.xx - 3;  // {4,4}
  printf("%d %d %d %d ", w.y, w.x, w.w, w.z);
  i4 e = {2, 5, 8, 11};
  printf("%d %d %d ", e.even.x, e.even.y, e.lo.y + e.s1 - 2);
  _Complex float c = (_Complex float){1.0f, 2.0f};
  printf("%g %g\n", __real__ c, __imag__ c);
  return 0;
}
