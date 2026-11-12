// Specialization forms a precompiled header's image carries (WP-1352).
#pragma once
namespace d1 {
template <class T> struct ch;
template <> struct ch<char>;
struct other { int k = 1; };
template <> struct ch<char> { static constexpr int size = 1; };
}
namespace d2 {
template <class A, class B> struct pr { static constexpr int v = 0; };
template <class A> struct pr<A, A>;
template <class A> struct pr<A, A> { static constexpr int v = 2; };
}
namespace d3 {
template <class T> inline constexpr T e_v = T(2.5);
inline constexpr double e = e_v<double>;
template <class T> inline constexpr int sz = sizeof(T);
inline int total() { return sz<int> + sz<char>; }
}
namespace d4 {
template <class T> struct box { T t; };
template <template <class> class C> int wrap() { return sizeof(C<int>); }
inline int use() { return wrap<box>(); }
}
namespace d5 {
template <class T> T inc(T t) { return t + 1; }
template <> int inc<int>(int t) { return t - 1; }
template <class T> T dbl(T t) { return t + t; }
extern template double dbl<double>(double);
template <class T> int ovl(T) { return 1; }
template <class T> int ovl(T *) { return 2; }
inline int pick() { int i = 0; return ovl(&i) + inc(1); }
}
