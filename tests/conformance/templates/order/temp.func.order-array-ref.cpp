// [temp.func.order]: f(T(&)[N]) is more specialized than f(T&).
template <class T> constexpr int f(T&) { return 1; }
template <class T, int N> constexpr int f(T (&)[N]) { return 2 + N; }
int a[3];
int i;
static_assert(f(a) == 5);
static_assert(f(i) == 1);
