#ifndef PREDEF_ALIGNED_H
#define PREDEF_ALIGNED_H
// The shapes of <__locale_t.h>, <__stddef_max_align_t.h> and <limits>.
namespace pa {
struct S {
  struct D *p[2];
  int k;
};
struct D {
  int v;
};
typedef struct {
  long long a __attribute__((__aligned__(__alignof__(long long))));
  long double b __attribute__((__aligned__(__alignof__(long double))));
} max_align;
struct W {
  char c __attribute__((__aligned__(8)));
  char d __attribute__((aligned));
};
template <class T> struct L {
  static const int bits = sizeof(T) * __CHAR_BIT__;
};
template <class T> struct Big {
  static_assert(sizeof(T) * __CHAR_BIT__ >= 32, "too small");
  static const int max = __INT_MAX__ / sizeof(T);
};
template <class T> constexpr int add(T n) {
  return __INT_MAX__ + n;
}
template <class T> T trunc() {
  T a = __INT_MAX__;
  return a;
}
template <class T> struct is_wide {
  static const bool value = false;
};
template <> struct is_wide<__int128_t> {
  static const bool value = true;
};
template <> struct is_wide<__uint128_t> {
  static const bool value = true;
};
typedef __builtin_va_list va;
int vsum(int n, va ap);
struct jb;
int count(struct jb *e[1]);
inline int third(const int a[3]) { return a[2]; }
template <class T> T second(T, const int a[2], T (*f)(T)) { return f(a[1]); }
} // namespace pa
#endif
