// Interface unit of module `imgb` for the module.interface image body
// tests (spec/modules.md §11.11 "Part 2a", stage S2): inline, constexpr
// and consteval function bodies and variable initializers, every one of
// which the declaration image carries, so under `-fkiln-bmi-image` an
// importer evaluates and inlines them from the image alone.
export module imgb;
namespace imgb {
export constexpr int fact(int n) {
  int r = 1;
  for (int i = 2; i <= n; ++i)
    r *= i;
  return r;
}
export consteval int sq(int x) { return x * x; }
export inline int clamp3(int x, int lo = 0, int hi = 10) {
  return x < lo ? lo : (x > hi ? hi : x);
}
export constexpr int classify(int x) {
  switch (x) {
  case 0:
    return 100;
  case 1:
  case 2:
    x += 10;
    [[fallthrough]];
  case 3: {
    int y = x * 2;
    return y;
  }
  default:
    break;
  }
  return -1;
}
export constexpr int loops(int n) {
  int a = 0, k = 0;
  while (k < n) {
    ++k;
    if (k % 2 == 0)
      continue;
    a += k;
  }
  do {
    a -= 1;
  } while (a > 100);
  int arr[4] = {1, 2};
  for (int v : arr)
    a += v;
  return a;
}
export constexpr const char *kName = "kiln";
export constexpr int length(const char *s) {
  int n = 0;
  while (s[n] != '\0')
    n++;
  return n;
}
export constexpr long mix(int x) {
  long l = static_cast<long>(x) + (long)x + long(x);
  double d = 1.5;
  bool b = x > 0 && true;
  const int &r = 4;
  int *p = nullptr;
  return l + static_cast<long>(d * 2) + b + r + (p == nullptr) + sizeof(int) / alignof(int) +
         noexcept(x + 1) + int() + (x, 2) + __is_integral(int);
}
export inline int jump(int x) {
  int n = 0;
again:
  n += x;
  if (n < 10)
    goto again;
  return n;
}
export inline const char *fname() { return __func__; }
export inline int stmt_expr(int x) {
  return ({
    int t = x * 3;
    t + 1;
  });
}
export constexpr int with_default(int a, int b = sq(3)) { return a + b; }
export constexpr int call_default(int a) { return with_default(a); }
export inline int checked(int x) {
  if (x < 0)
    throw x;
  return x;
}
export constexpr unsigned flip(unsigned u) { return ~u & 0xffu; }
export constexpr char upper(char c) { return c >= 'a' && c <= 'z' ? char(c - 'a' + 'A') : c; }
export constexpr int kAnswer = fact(5);
export inline int counter = 3;
export const int kLimit = 7;
export constexpr int kArr[3] = {4, 5, 6};
export constexpr double kHalf = 0.5;
export constexpr bool kOn = kAnswer > 100;
} // namespace imgb
