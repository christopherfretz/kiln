// [temp.deduct.type]/8, [temp.spec.partial.match]/2: template arguments
// deduced through nested template-ids and through an array bound inside
// a template argument.
template <class T> struct box {};
template <class T, int N> struct arr {};
template <class T> struct unwrap { static constexpr int depth = 0; };
template <class T> struct unwrap<box<T>> {
  static constexpr int depth = 1 + unwrap<T>::depth;
};
template <class T, int N> struct unwrap<box<arr<T, N>>> {
  static constexpr int depth = 100 + N;
};
template <class T, int N> struct unwrap<box<T[N]>> {
  static constexpr int depth = 200 + N;
};
static_assert(unwrap<int>::depth == 0);
static_assert(unwrap<box<box<box<int>>>>::depth == 3);
static_assert(unwrap<box<arr<char, 7>>>::depth == 107);
static_assert(unwrap<box<box<arr<char, 2>>>>::depth == 103);
static_assert(unwrap<box<long[5]>>::depth == 205);
