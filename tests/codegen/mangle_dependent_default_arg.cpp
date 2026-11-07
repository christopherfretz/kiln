// A class template specialization in the qualifier of a dependent name in a
// function template's signature is mangled with its arguments as written,
// without the defaulted ones the canonical type carries (clang
// `mangleUnresolvedTypeOrSimpleId`; [temp.arg.general]/9,
// [temp.over.link]/5): `R<T>::value` is `sr1RIT_EE5value`, `W<T, long>` is
// `WIT_lEE`, and a default written out (`Z<T, char>`) stays (`ZIT_cEE`). This is libstdc++'s `__enable_if_t<__is_bitwise_relocatable<T>::value, T*>`
// shape. g++ 16.1 differs here (`sr1RIT_vE5value`, no closing `E`); kiln
// follows clang.
template <bool B, class T = void> struct en {};
template <class T> struct en<true, T> { using type = T; };
template <class T, class = void> struct R { static constexpr bool value = true; };
template <class T, class U = int, class V = T *> struct W { static constexpr bool value = true; };
template <class T> typename en<R<T>::value, T *>::type f(T *p) { return p; }
template <class T> typename en<W<T>::value, T *>::type g(T *p) { return p; }
template <class T> typename en<W<T, long>::value, T *>::type h(T *p) { return p; }
template <class T, class = char> struct Z { static constexpr bool value = true; };
template <class T> typename en<Z<T, char>::value, T &>::type k(T &p) { return p; }
int *use(int *p) { return f(p) + (g(p) - h(p)) + (&k(*p) - p); }
