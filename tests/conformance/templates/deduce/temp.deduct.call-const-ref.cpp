// [temp.deduct.call]/3: P = const T& binds rvalues; T is the unqualified type.
template <class A, class B> struct same { static constexpr bool v = false; };
template <class A> struct same<A, A> { static constexpr bool v = true; };
template <class T> constexpr bool is_int(const T&) { return same<T, int>::v; }
const int ci = 4;
static_assert(is_int(1));
static_assert(is_int(ci));
static_assert(!is_int(1L));
