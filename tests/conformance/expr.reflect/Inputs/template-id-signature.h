// Templates whose signatures hold a dependent reflect-expression of a
// template-id (WP-1250): in a decltype return type, in a parameter type
// through a dependent template argument, and in a requires-clause.
using info = decltype(^^int);
template <info R> struct K {};
template <class T> struct TT {};
template <class T> using AT = TT<T*>;
template <class T, class U> using F = TT<T>;
template <class T, int N> using Arr = TT<T[N]>;
template <class T> struct RC { using type = T; };
template <class T> struct RC<const T> { using type = T; };
template <class T> using D = typename RC<T>::type;
template <class T> constexpr int vt = 0;
template <class T> constexpr int nv = 1;
template <class T> int g(T) { return 0; }
template <class T> int h(T) { return 0; }
namespace ns {
struct A {};
template <class T> bool operator==(A, T) { return true; }
template <class T> constexpr int nv = 0;
}
template <class T> struct S { template <class U> static int m(U) { return 0; } };
