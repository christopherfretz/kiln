// [temp.constr.decl]/3, [temp.constr.atomic]/3, [temp.dep.expr]/3: a
// requires-clause of a member of a class template that calls a static
// member function template of the class with non-dependent explicit
// arguments (`ok<int>()`, `ok<>()`) is checked against that member of
// the specialization, not the pattern's (libstdc++ tuple:965
// `__constructible<const _Elements&...>()` with an empty pack; WP-332).
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 3
extern "C" int printf(const char *, ...);
template<class E> struct T {
  template<class U> static consteval bool ok() { return sizeof(U) <= sizeof(E); }
  int n = 0;
  T() requires (ok<int>()) : n(1) {}
  T(int) requires (!ok<int>()) : n(2) {}
};
template<class... E> struct V {
  template<class... U> static consteval bool ok() { return sizeof...(U) == sizeof...(E); }
  int n = 0;
  V() requires (ok<>()) : n(3) {}
};
int main() {
  T<long long> a;
  T<char> b(0);
  V<> c;
  printf("%d %d %d\n", a.n, b.n, c.n);
}
