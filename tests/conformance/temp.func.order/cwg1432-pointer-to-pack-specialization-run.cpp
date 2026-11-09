// [temp.func.order], CWG1432: `g(L<T>*)` is more specialized than
// `g(L<T, U...>*)`, the trailing pack nested under a pointer (WP-1105).
// clang 19 reports the call ambiguous; clang 22 and kiln pick `L<T>*`.
//@ std: c++20
//@ expect: run
//@ requires: clang>=22
//@ stdout: 2 1
extern "C" int printf(const char *, ...);
template <class... T> struct L {};
template <class T, class... U> constexpr int g(L<T, U...> *) { return 1; }
template <class T> constexpr int g(L<T> *) { return 2; }

int main() { printf("%d %d\n", g((L<int> *)nullptr), g((L<int, int> *)nullptr)); }
