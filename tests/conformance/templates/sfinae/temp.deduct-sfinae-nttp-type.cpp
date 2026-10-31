// [temp.deduct]/11: a non-type argument of an invalid type fails.
template <class T, typename T::type N = 0> constexpr int f(T) { return 1; }
constexpr int f(...) { return 2; }
struct A { using type = int; };
static_assert(f(A{}) == 1);
static_assert(f(0) == 2);
