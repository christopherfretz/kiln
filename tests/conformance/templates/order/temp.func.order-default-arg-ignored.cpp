// [temp.func.order]/3: only parameters with arguments take part in ordering.
template <class T> constexpr int f(T, int = 0) { return 1; }
template <class T> constexpr int f(T*) { return 2; }
int x;
static_assert(f(&x) == 2);
