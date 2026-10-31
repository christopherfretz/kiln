// [temp.deduct]/8: a substitution failure removes the candidate silently.
struct A { using type = int; };
template <class T> constexpr int f(typename T::type) { return 1; }
template <class T> constexpr int f(T) { return 2; }
static_assert(f<A>(0) == 1);
static_assert(f(0) == 2);
