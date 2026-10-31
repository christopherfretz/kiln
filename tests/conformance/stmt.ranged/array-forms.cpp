// [stmt.ranged]/1.1: an array range iterates from `__range` to
// `__range + bound`; the loop variable forms `auto`, `auto&`, `auto&&` and
// `const auto&`, a declared type, and `break`/`continue` in the body.
//@ expect: run
//@ stdout: 15 30 60 60
//@ stdout: 1 3 4
extern "C" int printf(const char*, ...);

int main() {
  int a[5] = {1, 2, 3, 4, 5};
  int s1 = 0;
  for (auto x : a) s1 += x;
  for (auto& x : a) x *= 2;
  int s2 = 0;
  for (auto&& x : a) s2 += x;
  for (int& x : a) x += x;
  int s3 = 0;
  for (const auto& x : a) s3 += x;
  long s4 = 0;
  for (long x : a) s4 += x;
  printf("%d %d %d %ld\n", s1, s2, s3, s4);
  int b[] = {1, 2, 3, 4, 5, 6};
  for (int x : b) {
    if (x == 2) continue;
    if (x == 5) break;
    printf(x == 4 ? "%d\n" : "%d ", x);
  }
}
