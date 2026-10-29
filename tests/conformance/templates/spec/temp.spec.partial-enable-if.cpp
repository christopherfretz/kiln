// [temp.spec.partial.match]/2, [temp.deduce]/8: enable_if-style partial
// specializations selected by a constant condition.
template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
template <class T, class = void> struct kind { static constexpr int v = 0; };
template <class T> struct kind<T, typename enable_if<(sizeof(T) > 4)>::type> {
  static constexpr int v = 1;
};
static_assert(kind<char>::v == 0);
static_assert(kind<int>::v == 0);
static_assert(kind<long>::v == 1);
static_assert(kind<double>::v == 1);
