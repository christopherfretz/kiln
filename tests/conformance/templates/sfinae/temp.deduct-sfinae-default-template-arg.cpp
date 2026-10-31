// [temp.deduct]/8: a failing default template argument removes the candidate.
template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
template <class T> struct is_int { static constexpr bool v = false; };
template <> struct is_int<int> { static constexpr bool v = true; };
template <class T, class = typename enable_if<is_int<T>::v>::type> constexpr int f(T) { return 1; }
constexpr int f(...) { return 2; }
static_assert(f(1) == 1);
static_assert(f(1.0) == 2);
