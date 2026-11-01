// [temp.variadic]/5, [expr.prim.lambda]: a lambda whose body names a pack
// of the enclosing template is part of the pattern of the enclosing pack
// expansion, which is expanded once per element (a fold expression or a
// braced initializer list); the pack may appear only in a declaration or
// a statement of the body. Verified against clang 19.
//@ expect: run
template <class... A> struct S {
  static constexpr int a[sizeof...(A)]{[] { return (int)sizeof(A); }()...};
  static constexpr int b[sizeof...(A)]{[] { using T = A; return (int)sizeof(T); }()...};
};
static_assert(S<char, int>::a[0] == 1 && S<char, int>::a[1] == 4);
static_assert(S<char, int, short>::b[2] == 2);
template <class... A> constexpr int f() {
  int r = 0;
  ((r += [] { A x{}; return (int)sizeof(x); }()), ...);
  ((r += [] { if (sizeof(A) > 1) return 100; return 0; }()), ...);
  return r;
}
static_assert(f<char, short>() == 103);
template <class... A> int g(A... a) {
  int r = 0;
  ((r = r * 10 + [&] { return (int)a; }()), ...);
  return r;
}
int main() { return g(1, 2, 3) == 123 ? 0 : 1; }
