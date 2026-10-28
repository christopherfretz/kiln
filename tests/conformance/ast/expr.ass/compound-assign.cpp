//@ expect: ast
void ops(int i, double d, unsigned u, int *p, char c) {
  i += 2;
  i -= c;
  d *= i;
  u /= 3;
  i %= 5;
  u <<= 1;
  i >>= 2;
  u &= i;
  u |= 1;
  u ^= 2;
  p += 3;
  c += d;
}
