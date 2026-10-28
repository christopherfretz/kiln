//@ expect: ast
unsigned bits(unsigned u, int i, char c) {
  unsigned a = u << 2;
  int b = i >> c;
  unsigned m = (u & 0xFF) | (i ^ 3);
  return a + b + m + ~u;
}
