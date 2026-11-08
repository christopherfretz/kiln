// Mangling differential: lambda closure types (spec/abi.md §8.15,
// WP-818; [expr.prim.lambda.closure]). A generic lambda's
// `<lambda-sig>` lists its explicit template parameters (`Ty`, `Tn`,
// `Tk`, `Tp`) and the head requires-clause before the parameter types,
// and the discriminator counts closures with the same signature, so
// `[]<class T>(T)` and `[](auto)` are numbered separately.

template <class T> concept C = sizeof(T) > 0;
template <class F> inline int call(F f) { return f(1); }
inline int g() {
  auto a = []<class T>(T& x) { return x; };
  auto b = []<C T, class... U>(T x, U...) { return x; };
  auto c = []<int N>(int x) { return x + N; };
  auto d = []<class T> requires C<T> (T x) { return x; };
  auto e = [](auto x) { return x; };
  int i = 1;
  return a(i) + b(1, 2) + c.operator()<3>(1) + d(1) + e(1) + call([]<class T>(T x) { return x; });
}
int h() { return g(); }
inline int k() {
  auto a = [](auto x) { return x; };
  auto b = [](auto x) { return x + 1; };
  auto c = []<class T>(T x) { return x; };
  auto d = []<class T>(T x) { return x + 1; };
  auto e = [](int x) { return x; };
  return a(1) + b(1) + c(1) + d(1) + e(1);
}
int m() { return k(); }
