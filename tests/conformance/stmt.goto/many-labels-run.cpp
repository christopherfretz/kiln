// [stmt.label]/1, [stmt.goto]/1: a function with a long run of labels,
// each the target of a `goto` written after it, jumps to the label named
// (the label searches are hashed since WP-2478; a run of labels cost
// O(N^2) before). The labels are l000 .. l077 and the values the octal
// literals 000 .. 077 (0 .. 63).
//@ expect: run
//@ stdout: 2016 -2 87360
extern "C" int printf(const char*, ...);
#define STEP(n) l##n : sum += n; if (n == 077) goto out;
#define PICK(n) if (k == n) goto l##n;
#define R8(M, a) M(a##0) M(a##1) M(a##2) M(a##3) M(a##4) M(a##5) M(a##6) M(a##7)
int walk(int k) {
  int sum = 0;
  goto pick;
  R8(STEP, 00) R8(STEP, 01) R8(STEP, 02) R8(STEP, 03)
  R8(STEP, 04) R8(STEP, 05) R8(STEP, 06) R8(STEP, 07)
pick:
  R8(PICK, 00) R8(PICK, 01) R8(PICK, 02) R8(PICK, 03)
  R8(PICK, 04) R8(PICK, 05) R8(PICK, 06) R8(PICK, 07)
  return -2;
out:
  return sum;
}
int main() {
  int total = 0;
  for (int k = 0; k < 64; ++k) total += walk(k);
  printf("%d %d %d\n", walk(0), walk(-1), total);
  return 0;
}
