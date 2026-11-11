export module imgu;
export namespace iu {
constexpr int f(int x) { return x + 1; }
constexpr long f(long x) { return x + 2; }
template <class T> constexpr T g(T x) { return x * 2; }
template <int N> constexpr int k() { return N; }
template <class T> constexpr auto call(T t) { return f(t); }
template <class T> constexpr auto qual(T t) { return iu::f(t); }
template <class T> constexpr auto expl(T t) { return iu::g<T>(t); }
template <int N> constexpr int next() { return k<N + 1>(); }
template <class T> constexpr int size = sizeof(T);
template <class T> constexpr int sz() { return size<T> + 3; }
struct S { int v; };
constexpr int h(S s) { return s.v; }
constexpr S operator+(S a, S b) { return S{a.v + b.v}; }
template <class T> constexpr int adl(T t) { return h(t); }
template <class T> constexpr auto plus(T a, T b) { return a + b; }
template <class... Ts> constexpr int sum(Ts... xs) { return (0 + ... + f(xs)); }
}
