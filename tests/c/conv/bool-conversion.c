// C17 6.3.1.2p1: converting a scalar to _Bool yields 0 if it compares
// equal to 0 and 1 otherwise (pointers and floating values included).
//@ expect: run
//@ stdout: 1 0 1 1 1 0
int printf(const char *, ...);
int main(void) {
  int v = 7;
  _Bool a = 256;
  _Bool b = 0.0;
  double q = 0.25;
  int *pv = &v;
  _Bool c = q;
  _Bool d = pv;
  _Bool e = -1;
  void *n = 0;
  _Bool f = n;
  printf("%d %d %d %d %d %d\n", a, b, c, d, e, f);
  return 0;
}
