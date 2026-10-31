// [temp.deduct.call]/2.2: a function argument decays to a pointer for a non-reference P.
int g(int x) { return x; }
template <class T> struct tag { static constexpr int v = 0; };
template <class R, class A> struct tag<R (*)(A)> { static constexpr int v = 1; };
template <class T> constexpr int kind(T) { return tag<T>::v; }
static_assert(kind(g) == 1);
static_assert(kind(&g) == 1);
