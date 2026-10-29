// [temp.dep.constexpr]/2, [temp.inst]/3: a variable template initialized
// from a static member of a class template specialization.
template <class T, T v> struct constant { static constexpr T value = v; };
template <class T> struct is_char : constant<bool, false> {};
template <> struct is_char<char> : constant<bool, true> {};
template <class T> constexpr bool is_char_v = is_char<T>::value;
static_assert(is_char_v<char>);
static_assert(!is_char_v<int>);
template <class T> constexpr int twice = constant<int, sizeof(T)>::value * 2;
static_assert(twice<int> == 8);
