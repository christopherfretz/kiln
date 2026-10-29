// [basic.fundamental]: unsigned arithmetic is modulo 2^N, with N the width
// of the type; __int128 holds 128-bit values.
//@ expect: run
//@ stdout: 0 255 65535 18446744073709551615
//@ stdout: 1 1
extern "C" int printf(const char*, ...);

int main() {
  unsigned char c = 255;
  ++c;
  unsigned char d = 0;
  --d;
  unsigned short s = 0;
  s -= 1;
  unsigned long long u = 0;
  u -= 1;
  printf("%d %d %d %llu\n", c, d, s, u);
  unsigned __int128 big = u;
  big += 2;
  unsigned long long hi = big >> 64, lo = big;
  printf("%llu %llu\n", hi, lo);
}
