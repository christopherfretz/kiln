// [temp.alias]/2, [temp.dep.constexpr]/2: an alias template whose pattern
// names a qualified member of a dependent class (`IsSmall<T>::value`) is
// substituted with a dependent argument; the reference stays dependent
// until the enclosing class template is instantiated.
template <bool B, class T = void> struct EnableIf { using type = T; };
template <bool B, class T = void> using EnableIfT = typename EnableIf<B, T>::type;
template <class T> struct IsSmall { static constexpr bool value = sizeof(T) < 4; };
template <class T> using SmallT = EnableIfT<IsSmall<T>::value && IsSmall<T>::value, T>;
template <class T> struct H { SmallT<T> x; };
static_assert(sizeof(H<char>) == 1);
