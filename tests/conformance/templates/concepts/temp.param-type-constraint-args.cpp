//@ expect: compile-ok
// [temp.param]/4: `C<U> T` introduces C<T, U>.
template <class T, class U> concept Same = __is_same(T, U);
template <Same<int> T> constexpr int f(T) { return 1; }
template <Same<char> T> constexpr int f(T) { return 2; }
static_assert(f(1) == 1);
static_assert(f('a') == 2);
