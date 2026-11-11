// C17 6.5.16.2p3: a compound assignment `E1 op= E2` needs only an
// arithmetic left operand, so an enum (`flags |= X`, `e += 1`) and a real
// left operand with a complex right operand are valid in C; the result is
// converted back to the type of E1 (6.5.16.1p2). C++ rejects both.
//@ std: c17
//@ expect: run
//@ stdout: 3 4 2 | 3.5 7
int printf(const char *, ...);
typedef enum { A = 1, B = 2, C = 4 } flags_t;
int main(void) {
  flags_t f = 0;
  f |= A;
  f |= B;
  flags_t g = f;
  g += 1;
  flags_t h = C;
  h >>= 1;
  double d = 1.5;
  d += 2.0 + 3.0i;
  int n = 2;
  n *= 3.5 + 1.0i;
  printf("%d %d %d | %.1f %d\n", (int)f, (int)g, (int)h, d, n);
  return 0;
}
