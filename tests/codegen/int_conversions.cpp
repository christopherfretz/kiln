// Integer conversions: truncation, sign and zero extension, unsigned
// arithmetic and comparisons.
int conv(long l, unsigned u, short s, unsigned char uc, signed char sc) {
  int a = (int)l;
  long b = u;
  long c = s;
  unsigned d = sc;
  int e = uc;
  unsigned long long f = (unsigned long long)l * u;
  bool g = u < (unsigned)s;
  return a + (int)b + (int)c + (int)d + e + (int)f + g;
}
