// The module of written-bound-module-run.cpp.
export module written_bounds_mod;
export constexpr int f(int x) { return x; }
export int arr[f(2)] = {1, 2};
export template <class T> struct TA { T a[f(5)]; };
export template <class T, int N> struct P { static constexpr int k = 0; };
template <class T> struct P<T, f(1)> { static constexpr int k = 1; };
export template <class T, int N> constexpr int v = 0;
template <class T> constexpr int v<T, f(3)> = 3;
template <> constexpr int v<int, f(4)> = 4;
export int by_ref(int (&a)[f(2)]) { return a[1]; }
