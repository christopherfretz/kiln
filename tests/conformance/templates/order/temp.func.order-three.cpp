// [temp.func.order]: the most specialized of three templates.
template <class T> constexpr int f(T) { return 1; }
template <class T> constexpr int f(T*) { return 2; }
template <class T> constexpr int f(const T*) { return 3; }
int x;
const int y = 0;
static_assert(f(0) == 1);
static_assert(f(&x) == 2);
static_assert(f(&y) == 3);
