// [conv.integral] + [conv.prom]: conversion to an unsigned type is modulo
// 2^N, conversion to a signed type is modulo 2^N (C++20), and narrower
// types are promoted (sign- or zero-extended) before arithmetic.
//@ expect: run
//@ stdout: 255 -1 44 -56
//@ stdout: 4294967295 18446744073709551615 65535 -32768
extern "C" int printf(const char*, ...);

int main() {
  int m1 = -1;
  unsigned char uc = m1;
  signed char sc = m1;
  int big = 300;
  unsigned char t = big;
  int two_hundred = 200;
  signed char n = two_hundred;
  printf("%d %d %d %d\n", uc, sc, t, n);
  unsigned u = m1;
  unsigned long long ull = m1;
  unsigned short us = m1;
  int k32 = 32768;
  short s = k32;
  printf("%u %llu %d %d\n", u, ull, us, s);
}
