// C23 6.4.4.6 (nullptr, nullptr_t), 6.7.2 (bool/true/false keywords),
// 6.7.2.5 (typeof, typeof_unqual), 6.7.10 (static_assert without a
// message), 6.7.11 (empty initializer = {}), 6.7.1 (constexpr objects),
// 6.7.10 auto type inference, 6.7.13 ([[attributes]]), 6.9.2 (unnamed
// parameters in a definition), 6.8.2 (labels before declarations).
//@ expect: run
//@ std: c23
//@ stdout: 1 1 5 0 0 3 4
int printf(const char *, ...);
typedef typeof(nullptr) np_t;
static_assert(sizeof(np_t) == sizeof(void *));
static_assert(true && !false);
constexpr int K = 3;
[[nodiscard]] static int f(int, int b) { return b; }
int main(void) {
  bool b = true;
  int *p = nullptr;
  const int ci = 5;
  typeof_unqual(ci) m = ci;
  m = m;
  typeof(ci) *pc = &ci;
  struct S { int a, b; } s = {};
  int arr[3] = {};
  auto x = K + 1;
  goto end;
end:
  int r = f(0, x);
  printf("%d %d %d %d %d %d %d\n", b, p == nullptr, *pc, s.a + s.b, arr[2], K, r);
  return 0;
}
