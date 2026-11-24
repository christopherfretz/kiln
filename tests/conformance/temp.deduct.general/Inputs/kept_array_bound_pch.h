// Templates whose signatures keep instantiation-dependent array bounds
// (kept-array-bound-pch.cpp).
struct X {};
constexpr int f(int) { return 0; }
template <class T> constexpr int g(T, char (*)[sizeof(sizeof(f(T())))]) { return 1; }
constexpr int g(...) { return 2; }
template <class T> constexpr int k(T, void (*)(int[sizeof(f(T()))])) { return 1; }
constexpr int k(...) { return 2; }
template <class...> struct L {};
template <class... Ts> struct S { using t = L<char[sizeof(sizeof(Ts))]...>; };
