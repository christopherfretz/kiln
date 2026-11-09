// The module of image-template-run.cpp (spec/modules.md §11.11 stage S4):
// function templates with type and non-type parameters, whose patterns
// the declaration image carries.
export module imgt;
export namespace it {
constexpr long fib(long n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
template <class T> constexpr T add(T a, T b) { return a + b; }
template <class T, int N> constexpr T scale(T a) {
  T r = a * N;
  return r;
}
template <int N> constexpr int square() { return N * N; }
template <class T> T *addr(T &x) { return &x; }
template <typename T> T twice(T x) {
  T y = x;
  y += x;
  return y;
}
template <class T> constexpr int width() { return sizeof(T); }
template <class T> T narrow(long v) { return static_cast<T>(v); }
template <class T> T pick(T a, T b, bool first) { return first ? a : b; }
template <class T> T pick(T a, int) { return a; }
template <class T> T sum_to(T n) {
  T s = 0;
  for (T i = 1; i <= n; ++i)
    s += i;
  return s;
}
template <class T> T heavy(T x) {
  constexpr long k = fib(20);
  return x + k;
}
// Instantiated inside the interface: the importer's own instantiation
// of heavy<int> reuses the carried value of `k`.
int heavy_one() { return heavy(1); }
} // namespace it
