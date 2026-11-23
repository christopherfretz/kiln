// Templates whose signatures keep instantiation-dependent array bounds
// (kept-array-bound-module-image.cpp).
export module kept_array_bound_mod;
export struct X {};
export constexpr int f(int) { return 0; }
export template <class T> constexpr int g(T, char (*)[sizeof(sizeof(f(T())))]) { return 1; }
export constexpr int g(...) { return 2; }
export template <class T> constexpr int k(T, void (*)(int[sizeof(f(T()))])) { return 1; }
export constexpr int k(...) { return 2; }
export template <class T> constexpr int n(T, int a[sizeof(sizeof(f(T())))]) { return 1; }
export constexpr int n(...) { return 2; }
export template <class...> struct L {};
export template <class... Ts> struct S { using t = L<char[sizeof(sizeof(Ts))]...>; };
