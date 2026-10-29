// [temp.alias]/2, [temp.arg.nontype]/2: a non-type argument whose
// parameter's type is dependent (`template <class T, T V>` named with a
// dependent `T`) is converted when the enclosing template is instantiated.
template <class T, T V> struct IC { static constexpr T value = V; };
template <class T, T V> using ICA = IC<T, V>;
template <class T> struct U { static constexpr T v = ICA<T, 3>::value; ICA<T, 2> m[2]; };
static_assert(U<long>::v == 3);
static_assert(sizeof(U<char>) == 2);
