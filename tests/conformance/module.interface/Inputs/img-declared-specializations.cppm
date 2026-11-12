export module imgds;
export namespace ds {
template <class T> struct ch;
template <> struct ch<char>;
struct other { int k = 1; };
template <> struct ch<char> { static constexpr int size = 1; };
template <class A, class B> struct pr { static constexpr int v = 0; };
template <class A> struct pr<A, A>;
template <class A> struct pr<A, A> { static constexpr int v = 2; };
template <class T> inline constexpr T e_v = T(2.5);
inline constexpr double e = e_v<double>;
template <class T> struct box { T t; };
template <template <class> class C> int wrap() { return sizeof(C<int>); }
inline int use() { return wrap<box>(); }
template <class T> T inc(T t) { return t + 1; }
template <> int inc<int>(int t) { return t - 1; }
template <class T> T dbl(T t) { return t + t; }
extern template double dbl<double>(double);
template <class T> struct S {
  S() : v(0) {}
  int get() const { return v + 1; }
  int v;
};
extern template int S<int>::get() const;
inline int pick() { return inc(1) + dbl(2) + S<int>().get(); }
}
