// C17 6.3.1.1p2 (integer promotions), 6.3.1.8 (usual arithmetic
// conversions): unsigned char/short promote to int; unsigned int with
// long; the result types match C's rules.
//@ expect: run
//@ stdout: 1 1 1 1 1 -1 1
int printf(const char *, ...);
int main(void) {
  unsigned char uc = 200;
  unsigned short us = 65535;
  signed char sc = -1;
  unsigned u = 1;
  long l = -2;
  printf("%d %d %d %d %d %d %d\n",
         _Generic(uc + uc, int: 1, default: 0),
         _Generic(us * us, int: 1, default: 0),
         _Generic(u + l, long: 1, default: 0),
         _Generic(sc + 1u, unsigned: 1, default: 0),
         uc + uc == 400,
         (int)(sc < 1u ? 1 : -1),
         _Generic(1.0f + 1, float: 1, default: 0));
  return 0;
}
