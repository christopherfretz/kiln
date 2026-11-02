// [dcl.init.general]/16.6.2, [temp.dep.expr]/3, [temp.res.general]: in a
// template, `T x({a, b})` is kept for instantiation when `T` is dependent
// (the braced-init-list has no type of its own) or when an element of the
// list is type-dependent; the parenthesized braced-init-list then
// initializes the constructor parameter (libstdc++ <format>:439
// `_Utf32_view<ranges::subrange<iterator>> __uv({__first, __last})`,
// WP-323). Verified against clang 19.
//@ expect: run
//@ stdout: 1 2 3 1 2
extern "C" int printf(const char *, ...);

struct range {
  const char *b, *e;
  constexpr range(const char *x, const char *y) : b(x), e(y) {}
};
struct view {
  range r;
  constexpr explicit view(range &&x) : r(x) {}
};
template<class R> struct view_t {
  R r;
  constexpr explicit view_t(R &&x) : r(x) {}
};

int plain(const char *a, const char *b) { view v({a, b}); return v.r.e - v.r.b; }
// Non-dependent class, type-dependent elements.
template<class It> int elems(It a, It b) { view v({a, b}); return v.r.e - v.r.b; }
// Dependent class, type-dependent elements (the <format> shape).
template<class It> int both(It a, It b) { view_t<range> v({a, b}); return v.r.e - v.r.b; }

struct pt { int x, y; constexpr pt(int a, int b) : x(a), y(b) {} };
// Dependent class, non-dependent elements.
template<class T> constexpr int cls() { view_t<T> v({1, 2}); return v.r.y - v.r.x; }
template<class T> constexpr int type() { T v({1, 2}); return v.y; }
static_assert(cls<pt>() == 1 && type<pt>() == 2);

int main() {
  const char *s = "abc";
  printf("%d %d %d %d %d\n", plain(s, s + 1), elems(s, s + 2), both(s, s + 3), cls<pt>(), type<pt>());
}
