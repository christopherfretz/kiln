//@ expect: compile-ok
int conv(double d, char c) {
  int a = int(d);
  long b = long(c);
  unsigned u = unsigned(a);
  return a + int(b) + int(u);
}
