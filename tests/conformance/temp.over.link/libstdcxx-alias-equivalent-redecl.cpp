// [temp.over.link]/5-7, [temp.alias]/2: a function template declared with
// an alias-template return type (`_Require<...>`) and redeclared with the
// expansion of that alias (`typename enable_if<...>::type`) is the same
// template, so the call is not ambiguous. libstdc++ type_traits:2824 declares
// std::swap, bits/move.h:226 defines it; <streambuf>, <memory_resource>, ...
// 34 headers (docs/status/libstdcxx.md, WP-188).
// Verified against clang 19.
//@ expect: compile-ok
template<bool B, class T = void> struct enable_if {};
template<class T> struct enable_if<true, T> { using type = T; };
template<bool B, class T = void> using enable_if_t = typename enable_if<B, T>::type;
template<class... C> struct and_ { static constexpr bool value = (C::value && ...); };
template<class T> struct movable { static constexpr bool value = true; };
template<class T> struct copyable { static constexpr bool value = true; };
template<class... C> using Require = enable_if_t<and_<C...>::value>;
template<class T> Require<movable<T>, copyable<T>> swap(T &, T &);
template<class T>
typename enable_if<and_<movable<T>, copyable<T>>::value>::type swap(T &a, T &b) { T t = a; a = b; b = t; }
void f(int &a, int &b) { swap(a, b); }
