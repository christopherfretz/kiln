// Compound assignment on int, unsigned, double, char, pointer and
// through pointers and array elements.
int compound(int a, unsigned u, double d, char c, int* p, int* arr) {
  a += 3; a -= 1; a *= 2; a /= 3; a %= 5; a <<= 2; a >>= 1; a &= 0xff; a |= 4; a ^= 1;
  u += a; u >>= 3;
  d *= 1.5; d -= a; d /= 2;
  c += 1; c *= 2;
  p += 2; p -= 1;
  *p += 7;
  arr[a & 3] -= 2;
  return a + (int)u + (int)d + c + *p;
}
