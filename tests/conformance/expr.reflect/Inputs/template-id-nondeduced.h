// Alias templates whose parameters appear in a non-deduced context of the
// defining-type-id ([temp.deduct.type]/5: a nested-name-specifier, a
// decltype operand), for the reflected-alias mangling tests of WP-1260.
using info = decltype(^^int);
template <info R> struct K {};
template <class T> struct TT {};
template <class T, class U> struct TT2 {};
template <class T> struct RC { using type = T; };
template <class T> struct RC<const T> { using type = T; };
template <class T, class U> struct RC2 { using type = T; };
template <int N> struct RN { using type = int; };
template <class T> using D = typename RC<T>::type;
template <class T, class U> using D2 = typename RC2<T, U>::type;
template <class T, class U> using DP = TT2<typename RC<T>::type, U>;
template <class T, class U>
using DL = TT2<typename RC<T>::type, typename RC<U>::type>;
template <class T> using AT = TT<T*>;
template <class T> using N1 = D<AT<T>>;
template <class T> using N2 = AT<D<T>>;
template <class T> using N3 = D<D<T>>;
template <class T> using X = TT<D<T>>;
template <class T> using Z = D<T>*;
template <class... T> using DV = TT<typename RC<T>::type...>;
template <int N> using DN = typename RN<N>::type;
template <class T> using DT = decltype(T{});
template <class T> using NDS = decltype(sizeof(T));
template <class T> using ND = int;
