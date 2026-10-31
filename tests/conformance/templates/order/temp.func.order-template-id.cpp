// [temp.func.order]: f(box<T>) is more specialized than f(T).
template <class T> struct box {};
template <class T> constexpr int f(T) { return 1; }
template <class T> constexpr int f(box<T>) { return 2; }
static_assert(f(0) == 1);
static_assert(f(box<int>{}) == 2);
