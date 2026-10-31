// [temp.func.order]: f(T, T) is more specialized than f(T, U).
template <class T, class U> constexpr int f(T, U) { return 1; }
template <class T> constexpr int f(T, T) { return 2; }
static_assert(f(1, 2.0) == 1);
static_assert(f(1, 2) == 2);
