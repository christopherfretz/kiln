// [temp.deduct]/8: enable_if in the return type selects an overload.
template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
template <class T> constexpr typename enable_if<(sizeof(T) > 4), int>::type f(T) { return 8; }
template <class T> constexpr typename enable_if<(sizeof(T) <= 4), int>::type f(T) { return 4; }
static_assert(f('a') == 4);
static_assert(f(1) == 4);
static_assert(f(1.0) == 8);
