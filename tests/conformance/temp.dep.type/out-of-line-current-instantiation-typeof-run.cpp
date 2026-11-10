// [temp.dep.type]/1, /6, [temp.over.link]/5: GNU `typeof`/`__typeof__`
// and `__typeof_unqual__` before the declarator-id of an out-of-line
// member of a class template, of a type or of an expression, rebuilt in
// the current instantiation: `typename C<X>::T` and `C<X>::N` in the
// operand name the members, so the type is the in-class one, at the top
// level and as a template argument (clang's
// `RebuildDeclaratorInCurrentInstantiation` for `TST_typeofType` and
// `TST_typeofExpr`; WP-1169).
//@ expect: run
//@ strict-diags
//@ stdout: 7 5 4 4 3
extern "C" int printf(const char*, ...);
template <class X> struct W { static constexpr int v = sizeof(X); };
template <class X> struct C {
  using T = X;
  static constexpr int N = sizeof(X);
  T v = 7;
  __typeof__(T) a();
  __typeof__(N + 1) b();
  W<__typeof__(T{})> c();
  W<__typeof__(char[N])> d();
  __typeof_unqual__(const T) e();
};
template <class X> __typeof__(typename C<X>::T) C<X>::a() { return v; }
template <class X> __typeof__(C<X>::N + 1) C<X>::b() { return N + 1; }
template <class X> W<__typeof__(typename C<X>::T{})> C<X>::c() { return {}; }
template <class X> W<__typeof__(char[C<X>::N])> C<X>::d() { return {}; }
template <class X> __typeof_unqual__(const typename C<X>::T) C<X>::e() { return 3; }
int main() {
  C<int> c;
  printf("%d %d %d %d %d\n", c.a(), c.b(), decltype(c.c())::v, decltype(c.d())::v, c.e());
}
