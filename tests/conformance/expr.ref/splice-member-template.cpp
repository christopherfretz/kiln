// [expr.ref]/1, [expr.prim.splice]/3: `x.template [: r :]` and
// `x.template [: r :] < args >` name the member function template or
// static data member template `r` designates, of the object's class or a
// base: the designated template, not what its name finds in the object's
// class (a derived class's template of the same name does not hide it).
// Without arguments a function template's arguments are deduced by the
// call. In a template the access is substituted with the splice, and a
// member variable template-id `s.a<N>` keeps its arguments
// ([temp.names]/3). Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ oracle: gcc
struct S {
  template <typename U> constexpr int foo(int x, const U &) const { return 40 + x; }
  template <int N> static constexpr int a = N;
};
struct T : S {
  template <typename U> constexpr int foo(int x, const U &) const { return 50 + x; }
  template <int N> static constexpr int a = N + 100;
};
constexpr T t;
static_assert(t.template [:^^S::foo:](1, 1.0f) == 41);
static_assert(t.template [:^^T::foo:](1, 1.0f) == 51);
static_assert(t.template [:^^S::foo:]<double>(2, 1.0) == 42);
static_assert(t.template [:^^S::a:]<4> == 4);
static_assert(t.template [:^^T::a:]<4> == 104);
static_assert((&t)->template [:^^S::a:]<5> == 5);

template <int N> constexpr int f() {
  constexpr T u;
  static_assert(u.template [:^^S::foo:](N, 1.0f) == 40 + N);
  static_assert(u.template [:^^S::a:]<N> == N);
  static_assert(u.a<N> == N + 100);
  return u.template [:^^S::a:]<N + 1>;
}
static_assert(f<3>() == 4);

template <class X> constexpr int g(const X &x) {
  return x.template [:^^S::foo:](1, 'c') + x.template [:^^S::a:]<2>;
}
static_assert(g(t) == 43);
