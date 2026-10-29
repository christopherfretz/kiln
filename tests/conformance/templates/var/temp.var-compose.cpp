// [temp.pre]/1, [temp.names]: variable templates used in the initializers
// of other variable templates (the <type_traits> `_v` style).
template <class T, class U> constexpr bool is_same_v = false;
template <class T> constexpr bool is_same_v<T, T> = true;
template <class T> constexpr bool is_void_v = is_same_v<T, void>;
template <class T, class U, class V> constexpr bool one_of_v = is_same_v<T, U> || is_same_v<T, V>;
static_assert(is_void_v<void>);
static_assert(!is_void_v<int>);
static_assert(one_of_v<int, char, int>);
static_assert(!one_of_v<int, char, long>);
