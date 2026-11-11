// C23 6.2.5p8, 6.4.4.1: _BitInt(N) bit-precise integer types.
//@ expect: run
//@ std: c23
//@ stdout: 5 1
int printf(const char *, ...);
int main(void) {
  unsigned _BitInt(7) u = 127;
  u += 6;
  unsigned _BitInt(66) w = 1;
  w <<= 64;
  printf("%d %d\n", (int)u, w > 0);
  return 0;
}
