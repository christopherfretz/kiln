export module imgd;
export namespace id {
template <class T> struct box { using type = T; struct inner { using type = T *; }; };
template <class T> typename T::type get(const T &) { return typename T::type{}; }
template <class T> typename T::inner::type ptr(const T &) { return nullptr; }
template <class T> __remove_cv(T) rcv(T t) { return t; }
template <class T> using no_ref = __remove_reference_t(T);
struct S { int m; int f() { return m + 1; } };
using pm = int S::*;
template <class T, class C> T call(C &c, T (C::*p)()) { return (c.*p)(); }
inline int via(S &s, pm p) { return s.*p; }
typedef int v4 __attribute__((vector_size(16)));
inline int vsum(int a) { v4 v = {a, a, a, a}; return v[0] + v[3]; }
template <class T> constexpr bool val() { return T::value; }
template <class T> constexpr int sz() { return T::inner::size + T::count; }
template <class T> auto addr() { return &T::count; }
template <class T> struct Wrap { T v; Wrap(T x) { v = x; } Wrap(T *p) { v = *p; }
  Wrap(const char *) { v = 1; } };
template <class T> Wrap(T *) -> Wrap<T>;
Wrap(const char *) -> Wrap<int>;
template <class T> struct Agg { T a; T b; };
template <class T> Agg(T, T) -> Agg<long>;
template <class T> int deduce(T x) { Wrap w(x); return sizeof(w.v); }
inline int ctad() { int i = 5; Wrap w(&i); return w.v; }
}
