//@ expect: ast
long casts(int i, double d, void *v) {
  short s = (short)i;
  int t = (int)d;
  int *ip = (int *)v;
  bool b = (bool)ip;
  (void)b;
  return (long)s + t + (unsigned char)i;
}
