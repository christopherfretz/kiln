//@ expect: compile-ok
// [temp.constr.order], [temp.func.order]/6: the more constrained template
// is more specialized.
template <class T> concept Int = __is_same(T, int);
template <class T> concept Num = Int<T> || __is_same(T, double);
template <class T> constexpr int f(T) { return 0; }
template <Num T> constexpr int f(T) { return 1; }
template <Int T> constexpr int f(T) { return 2; }
static_assert(f('c') == 0);
static_assert(f(1.0) == 1);
static_assert(f(1) == 2);
