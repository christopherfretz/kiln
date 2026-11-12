// Block-scope static assertions in function templates, member functions
// of class templates and ordinary inline functions, and names with
// explicit template arguments (the shape of libc++'s
// <__utility/declval.h>, <cwchar> and <__bit/countl.h>; WP-1314).
#ifndef LOCAL_STATIC_ASSERT_H
#define LOCAL_STATIC_ASSERT_H
namespace lsa {
template <class T> int f(T t) {
  static_assert(sizeof(T) >= 1, "complete");
  static_assert(sizeof(T) < 64);
  return int(t) + 1;
}
inline int g(int x) {
  static_assert(sizeof(int) >= 2, "int");
  return x * 2;
}
template <class T> struct S {
  static int h() {
    static_assert(sizeof(T) != 3, "three");
    return 7;
  }
};
template <class T> T pick(T a) { return a; }
template <class T, int N> int scaled(T a) { return int(a) * N; }
inline int callpick() { return pick<int>(3) + scaled<long, 4>(2L); }
} // namespace lsa
#endif
