// [temp.deduct.call]/3: a reference P deduces from A with its cv-qualifiers.
template <class T> struct is_const { static constexpr bool v = false; };
template <class T> struct is_const<const T> { static constexpr bool v = true; };
template <class T> constexpr bool c(T&) { return is_const<T>::v; }
const int ci = 1;
int i = 2;
static_assert(c(ci));
static_assert(!c(i));
