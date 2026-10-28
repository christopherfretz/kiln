//@ expect: ast
double mix(int i, unsigned u, long l, float f, char c, short s) {
  long a = i * l;
  unsigned b = u / i;
  float g = f * i;
  int h = c + s;
  return a + b + g + h + l % i;
}
