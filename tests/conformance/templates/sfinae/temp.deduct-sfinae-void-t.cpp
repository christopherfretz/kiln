// [temp.deduct]/8: a void_t-style alias in a default argument.
template <class...> using void_t = void;
template <class T, class = void> struct has_x { static constexpr bool v = false; };
template <class T> struct has_x<T, void_t<decltype(T::x)>> { static constexpr bool v = true; };
template <class T, class = decltype(T::x)> constexpr int f(T) { return 1; }
constexpr int f(...) { return 2; }
struct X { static constexpr int x = 0; };
static_assert(f(X{}) == 1);
static_assert(f(1) == 2);
