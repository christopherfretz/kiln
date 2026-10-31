// [temp.func.order]: f(const T*) is more specialized than f(T*).
template <class T> constexpr int f(T*) { return 1; }
template <class T> constexpr int f(const T*) { return 2; }
int x;
const int y = 0;
static_assert(f(&y) == 2);
static_assert(f(&x) == 1);
