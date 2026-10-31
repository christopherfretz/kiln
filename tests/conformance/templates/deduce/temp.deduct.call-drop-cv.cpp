// [temp.deduct.call]/2.3: top-level cv of A is ignored for a non-reference P.
template <class T> struct is_const { static constexpr bool v = false; };
template <class T> struct is_const<const T> { static constexpr bool v = true; };
template <class T> constexpr bool c(T) { return is_const<T>::v; }
const int ci = 1;
static_assert(!c(ci));
