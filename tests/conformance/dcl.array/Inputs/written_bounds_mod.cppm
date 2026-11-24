// The module of written-bound-module-run.cpp.
export module written_bounds_mod;
export constexpr int f(int x) { return x; }
export int arr[f(2)] = {1, 2};
export template <class T> struct TA { T a[f(5)]; };
export template <class T, int N> struct P { static constexpr int k = 0; };
template <class T> struct P<T, f(1)> { static constexpr int k = 1; };
template <> struct P<int, f(2)> { static constexpr int k = 2; };
export template <class T, int N> constexpr int v = 0;
template <class T> constexpr int v<T, f(3)> = 3;
template <> constexpr int v<int, f(4)> = 4;
export int by_ref(int (&a)[f(2)]) { return a[1]; }
export template <int N> int fh() { return 0; }
template <> int fh<f(6)>() { return 6; }
export int par(int a[f(3)]) { return a[2]; }
export template <class T> struct O {
  template <class U, int N> struct In { static constexpr int k = 0; };
  template <class U> struct In<U, f(7)> { static constexpr int k = 7; };
};
