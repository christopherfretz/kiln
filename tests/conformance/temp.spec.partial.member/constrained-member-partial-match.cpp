// [temp.spec.partial.member]/2, [temp.spec.partial.match]/2: a
// constrained partial specialization of a member class template of a
// class template is matched in a specialization of the enclosing class
// with its constraints checked against the enclosing arguments and the
// deduced ones; constraints order the partials ([temp.func.order]).
// libc++ lazy_split_view.h:138 `template <forward_range _Tp> struct
// __outer_iterator_category<_Tp>` (WP-276).
//@ expect: compile-ok
//@ strict-diags
template <class T> concept big = sizeof(T) > 1;
template <class T> concept huge = big<T> && sizeof(T) > 4;
template <class V> struct view {
  template <class> struct cat { static constexpr int v = 0; };
  template <big T> struct cat<T> { static constexpr int v = 1; };
  template <huge T> struct cat<T> { static constexpr int v = 2; };
  template <class T>
    requires(sizeof(T) == sizeof(V))
  struct cat<T *> { static constexpr int v = 3; };
  template <bool C> struct it : cat<V> {};
};
static_assert(view<int>::cat<char>::v == 0);
static_assert(view<int>::cat<int>::v == 1);
static_assert(view<int>::cat<double>::v == 2);
static_assert(view<int>::cat<int *>::v == 3);
static_assert(view<char>::cat<int *>::v == 2);
static_assert(view<short>::it<true>::v == 1);
static_assert(view<char>::it<false>::v == 0);
