// [intro.object]/9, [dcl.attr.nouniqueaddr]/2, [expr.const]: an empty
// `[[no_unique_address]]` member shares its address with the next member;
// constant evaluation that subscripts the second member's array is not
// misled by the empty one at the same offset (the libc++ `layout_stride`
// mapping: empty `extents` followed by the `strides` array).
//@ expect: compile-ok
template <class T, unsigned long N> struct arr {
  T v[N];
  constexpr T &operator[](unsigned long i) { return v[i]; }
};
template <class T> struct arr<T, 0> {
  constexpr T &operator[](unsigned long) { __builtin_unreachable(); }
};
struct Stat { [[no_unique_address]] arr<unsigned, 0> d; };
struct Ext { [[no_unique_address]] Stat v; };
struct M {
  [[no_unique_address]] Ext e{};
  [[no_unique_address]] arr<unsigned, 2> s{};
  constexpr M() { s[1] = 3; s[0] = 25; }
};
constexpr bool t() { M m; return m.s[1] == 3 && m.s[0] == 25; }
static_assert(t());
static_assert(sizeof(M) == 8);
