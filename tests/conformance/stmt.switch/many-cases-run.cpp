// [stmt.switch]/2, /5: a switch of a long run of case labels (and of GNU
// case ranges) jumps to the case whose value equals the condition, and to
// `default` when none does (the duplicate check is hashed since WP-2478;
// a run of case labels cost O(N^2) before). The values are the octal
// literals 000 .. 077 (0 .. 63).
//@ expect: run
//@ args: -Wno-gnu-case-range
//@ stdout: 2016 -64 1 4096
extern "C" int printf(const char*, ...);
#define CASE(n) case n: return n;
#define RANGE(n) case 64 * n ... 64 * n + 63: return n;
#define R8(M, a) M(a##0) M(a##1) M(a##2) M(a##3) M(a##4) M(a##5) M(a##6) M(a##7)
int pick(int k) {
  switch (k) {
    R8(CASE, 00) R8(CASE, 01) R8(CASE, 02) R8(CASE, 03)
    R8(CASE, 04) R8(CASE, 05) R8(CASE, 06) R8(CASE, 07)
  default: return -1;
  }
}
int bucket(long k) {
  switch (k) {
    R8(RANGE, 00) R8(RANGE, 01) R8(RANGE, 02) R8(RANGE, 03)
    R8(RANGE, 04) R8(RANGE, 05) R8(RANGE, 06) R8(RANGE, 07)
  }
  return -1;
}
int main() {
  int sum = 0, miss = 0, b = 0;
  for (int k = 0; k < 128; ++k) {
    int r = pick(k);
    if (r < 0) miss += r; else sum += r;
  }
  for (long k = 0; k < 64 * 64; ++k) b += bucket(k) == k / 64;
  printf("%d %d %d %d\n", sum, miss, bucket(4095) == 63 && bucket(4096) == -1, b);
  return 0;
}
