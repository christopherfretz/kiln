// [expr.ass]: E1 op= E2 is E1 = E1 op E2 with E1 evaluated once, and the
// result is the left operand as an lvalue.
//@ expect: run
//@ stdout: 13 26 21 7 1 28 24 30 6
//@ stdout: 2 1 5
extern "C" int printf(const char*, ...);

int main() {
  int v = 10;
  int r[9];
  r[0] = (v += 3);
  r[1] = (v *= 2);
  r[2] = (v -= 5);
  r[3] = (v /= 3);
  r[4] = (v %= 2);
  r[5] = ((v = 7) <<= 2);
  r[6] = (r[5] & 0x1c ? (v = 24) : 0);
  r[7] = (v |= 6);
  r[8] = (v ^= 24);
  printf("%d %d %d %d %d %d %d %d %d\n", r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7], r[8]);
  int a[3] = {0, 0, 0};
  int i = 0;
  a[i++] += 2;
  (i += 1) += 3;
  a[1] = 1;
  printf("%d %d %d\n", a[0], a[1], i);
}
