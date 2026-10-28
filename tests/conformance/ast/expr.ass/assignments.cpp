//@ expect: ast
long assign(int a, long l, unsigned u) {
  a = 1;
  l += a;
  a *= 2;
  u <<= 3;
  u %= 5u;
  a = l = 4;
  a ^= u |= 1;
  return l;
}
