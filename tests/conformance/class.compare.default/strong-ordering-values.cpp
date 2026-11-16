// [cmp.categories.pre], [class.spaceship]: a strong ordering result only
// names `less`, `equal` and `greater`, so a library `strong_ordering`
// without `equivalent` is enough. Found running GCC's C++ testsuite
// (WP-1497).
//@ expect: compile-ok
//@ std: c++20
namespace std {
struct strong_ordering {
  int v;
  constexpr strong_ordering(int x) : v(x) {}
  constexpr operator int() const { return v; }
  static const strong_ordering less, equal, greater;
};
constexpr strong_ordering strong_ordering::less = -1;
constexpr strong_ordering strong_ordering::equal = 0;
constexpr strong_ordering strong_ordering::greater = 1;
}
struct B { int i; auto operator<=>(const B &) const = default; };
static_assert((B{1} <=> B{2}) == -1);
static_assert((B{2} <=> B{2}) == 0);
static_assert((3 <=> 2) == 1);
