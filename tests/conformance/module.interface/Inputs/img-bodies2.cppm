// Interface unit of module `imgc` for the module.interface image body
// tests (spec/modules.md §11.11 "Part 2a", stage S2): inline function
// bodies with static locals and further literal and operator kinds,
// imported by two translation units, which share one static local.
export module imgc;
export namespace imgc {
inline int next() {
  static int calls = 0;
  return ++calls;
}
inline const char *greeting() {
  static const char text[] = "hi";
  return text;
}
constexpr int widths() {
  return sizeof(L'x') + sizeof(u8'y') + sizeof(u'z') + sizeof(U'w') + sizeof(u8"ab") / 3;
}
constexpr int choose() { return __builtin_choose_expr(1, 7, 8.0); }
inline bool null_is_zero() { return __null == 0; }
constexpr int arrays(int n) {
  int a[5] = {};
  for (int i = 0; i < 5; ++i)
    a[i] += i * n;
  a[1] <<= 2;
  a[2] |= 1;
  a[3] -= 1;
  a[4] /= 2;
  return a[0] + a[1] + a[2] + a[3] + a[4];
}
constexpr double halves(double x) {
  float f = 0.25f;
  return x / 2 + f + 1e-1;
}
constexpr unsigned long long big() { return 0xffffffffffffffffull >> 4; }
constexpr int neg_mod(int a, int b) { return -a % b + (a > b) - !a + (~b & 3); }
constexpr int ptrs() {
  int a[3] = {1, 2, 3};
  int *p = a + 1;
  const int *q = &a[2];
  return *p + *q + (q - p) + (p < q);
}
const int kBound = 4;
constexpr const char *kWords = "abc" "def";
} // namespace imgc
