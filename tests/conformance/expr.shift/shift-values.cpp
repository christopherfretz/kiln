// [expr.shift]: E1 << E2 is E1 * 2^E2 modulo 2^N; E1 >> E2 is E1 / 2^E2
// rounded toward negative infinity (arithmetic for signed values).
//@ expect: run
//@ stdout: 40 2 -3 2147483648 -2147483648
//@ stdout: 4294967295 1
extern "C" int printf(const char*, ...);

int main() {
  int a = 5, n = 3;
  int m = -5;
  unsigned u = 1;
  int one = 1;
  printf("%d %d %d %u %d\n", a << n, a >> 1, m >> 1, u << 31, one << 31);
  unsigned all = ~0u;
  printf("%u %u\n", all >> 0, all >> 31);
}
