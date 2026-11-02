// __builtin_alloca and __builtin_alloca_with_align at run time: each call
// allocates a fresh, suitably aligned block in the calling frame where the
// call is (not hoisted to the function entry), live until the function
// returns. Verified against clang 19.
//@ expect: run
//@ stdout: 45 0 0 1
extern "C" int printf(const char *, ...);
int sum(int n) {
  int *prev = nullptr, total = 0;
  bool distinct = true;
  for (int i = 0; i < n; ++i) {
    int *p = static_cast<int *>(__builtin_alloca(sizeof(int) * (i + 1)));
    for (int j = 0; j <= i; ++j) p[j] = j;
    total += p[i];
    distinct = distinct && p != prev;
    prev = p;
  }
  return distinct ? total : -1;
}
int main() {
  void *a = __builtin_alloca(3);
  void *b = __builtin_alloca_with_align(5, 512);
  static_cast<char *>(b)[4] = 'x';
  printf("%d %d %d %d\n", sum(10), int((__UINTPTR_TYPE__)a % __BIGGEST_ALIGNMENT__),
         int((__UINTPTR_TYPE__)b % 64), static_cast<char *>(b)[4] == 'x');
}
