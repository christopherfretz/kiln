// clang's __builtin_memcpy_inline(dst, src, n) copies n bytes like memcpy
// (never as a library call), and __builtin_assume_separate_storage(a, b)
// is an optimizer hint with no effect on a correct program; both are
// declared, so __has_builtin answers 1 (WP-726, folly lang/CString.h and
// lang/Hint-inl.h).
//@ std: c++20
//@ expect: run
//@ strict-diags
//@ stdout: hello 7 42 1 1
#include <cstdio>
struct P { int x, y; };
template <unsigned N, class T> void copy(T *d, const T *s) {
  __builtin_memcpy_inline(d, s, N * sizeof(T));
}
int main() {
  static_assert(__has_builtin(__builtin_memcpy_inline));
  static_assert(__has_builtin(__builtin_assume_separate_storage));
  char buf[8] = {};
  __builtin_memcpy_inline(buf, "hello", 6);
  P a{7, 42}, b{};
  copy<1>(&b, &a);
  int u = 1, v = 1;
  __builtin_assume_separate_storage(&u, &v);
  volatile int w = 0;
  __builtin_assume_separate_storage(&w, buf);
  std::printf("%s %d %d %d %d\n", buf, b.x, b.y, u, v);
}
