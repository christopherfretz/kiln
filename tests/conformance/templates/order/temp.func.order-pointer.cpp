// [temp.func.order]: f(T*) is more specialized than f(T).
template <class T> constexpr int f(T) { return 1; }
template <class T> constexpr int f(T*) { return 2; }
int x;
static_assert(f(1) == 1);
static_assert(f(&x) == 2);
