// [temp.func.order]: f(R(*)(A)) is more specialized than f(T).
int g(int);
template <class T> constexpr int f(T) { return 1; }
template <class R, class A> constexpr int f(R (*)(A)) { return 2; }
static_assert(f(g) == 2);
static_assert(f(1.0) == 1);
