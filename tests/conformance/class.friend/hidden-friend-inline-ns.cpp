// [dcl.meaning.general]/2.3, [namespace.memdef]/3, [class.friend]/7: the
// lookup that decides whether an unqualified friend function redeclares an
// earlier declaration considers only the innermost enclosing namespace, not
// the members of its inline namespaces. libc++'s views declare a hidden
// friend `iter_move` in their iterators next to the `ranges::iter_move`
// customization point object (a variable in an inline namespace); the
// friend is found only by ADL, the variable by qualified lookup.
namespace ranges {
namespace cpo {
struct fn {
  template <class T> constexpr int operator()(T&&) const { return 1; }
};
} // namespace cpo
inline namespace cpos {
inline constexpr cpo::fn iter_move{};
}
template <class V> struct filter_view {
  struct iterator {
    int x;
    friend constexpr int iter_move(const iterator& i) { return i.x; }
    template <class U> friend constexpr int iter_swap(const iterator& i, U) { return i.x + 1; }
  };
};
inline namespace cpos {
inline constexpr cpo::fn iter_swap{};
}
struct plain {
  friend constexpr int iter_move(plain) { return 7; }
};
} // namespace ranges

constexpr ranges::filter_view<int>::iterator it{3};
static_assert(iter_move(it) == 3);
static_assert(iter_swap(it, 0) == 4);
static_assert(ranges::iter_move(it) == 1);
static_assert(ranges::iter_swap(it) == 1);
static_assert(iter_move(ranges::plain{}) == 7);
static_assert(ranges::iter_move(ranges::plain{}) == 1);
