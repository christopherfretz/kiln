// Coverage comparison program (scripts/coverage-compare.py): macros and
// a skipped #if 0 block.
#include <cstdio>
#define CHECK(c) do { if (!(c)) std::puts("fail: " #c); } while (0)
#define MAX(a, b) ((a) > (b) ? (a) : (b))
#if 0
int never() { return 1; }
#endif
static int sq(int x) { return x * x; }
int main(int argc, char **) {
  int m = MAX(argc, 2) + MAX(sq(argc), 0);
  CHECK(m > 0);
  CHECK(m < 0);
  for (int i = 0; i < 3; ++i) CHECK(i != 1);
  return m == 3 ? 0 : 1;
}
