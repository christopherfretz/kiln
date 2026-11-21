export module imgas;
export namespace as {
using as1_int = int __attribute__((address_space(1)));
template <int N> struct holder { using type = int __attribute__((address_space(N))); };
template <int N> using sp = int __attribute__((address_space(N)));
template <int N> int read(const int __attribute__((address_space(N))) *p) { return *p + N; }
template <int N> int which(int __attribute__((address_space(N))) *) { return N; }
template <int N> int next(int __attribute__((address_space(N + 1))) *p) { return *p * 10 + N; }
template <class T> int pick(T *) { return 0; }
template <class T> int pick(__attribute__((address_space(1))) T *) { return 1; }
inline int load(as1_int *p) { return *p; }
template <int N> struct box { static int __attribute__((address_space(N))) v; };
template <int N> int __attribute__((address_space(N))) box<N>::v = N * 3;
}
