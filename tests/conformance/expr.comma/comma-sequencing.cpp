// [expr.comma]: the left operand is evaluated (and discarded) before the
// right one, and the result is the right operand.
//@ expect: run
//@ stdout: 3 2 30
extern "C" int printf(const char*, ...);

int main() {
  int a = 1, b = 0;
  int c = (b = a + 1, b + 1);
  int s = 0;
  for (int i = 0, j = 10; i < 3; ++i, j += 0) s += j;
  printf("%d %d %d\n", c, b, s);
}
