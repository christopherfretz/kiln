// [stmt.break] + [stmt.cont]: break leaves the innermost loop; continue
// skips to the end of the innermost loop body.
//@ expect: run
//@ stdout: 1 3 5 7
//@ stdout: 0:0 1:0 1:1 2:0 2:1 2:2
extern "C" int printf(const char*, ...);

int main() {
  for (int i = 0;; ++i) {
    if (i % 2 == 0) continue;
    if (i > 7) break;
    printf(i == 7 ? "%d\n" : "%d ", i);
  }
  for (int i = 0; i < 3; ++i) {
    int j = 0;
    while (true) {
      printf("%d:%d", i, j);
      if (j++ == i) break;
      printf(" ");
    }
    printf(i == 2 ? "\n" : " ");
  }
}
