// [temp.func.order]: f(T C::*) is more specialized than f(T).
struct S { int m; };
template <class T> constexpr int f(T) { return 1; }
template <class T, class C> constexpr int f(T C::*) { return 2; }
static_assert(f(&S::m) == 2);
static_assert(f(0) == 1);
