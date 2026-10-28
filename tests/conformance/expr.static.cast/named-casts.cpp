//@ expect: compile-ok
int casts(double d, long l, int *p, const int *cp) {
  int a = static_cast<int>(d);
  long *lp = reinterpret_cast<long *>(p);
  int *mp = const_cast<int *>(cp);
  float f = static_cast<float>(l);
  return a + (int)*lp + *mp + (int)f;
}
