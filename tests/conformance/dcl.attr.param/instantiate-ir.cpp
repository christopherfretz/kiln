// [temp.inst]/3, [dcl.attr.grammar]/5: an instantiated function's
// parameters carry the attributes of the template's (clang
// `SubstParmVarDecl` → `InstantiateAttrs`), with dependent arguments
// substituted: `nonnull`, `noescape`, `align_value(N)`, `annotate("x",
// N)`, `maybe_undef`, `pass_object_size`, `aligned(N)`, of a
// function template, a member function of a class template, a member
// function template and a generic lambda, and of each element of a
// function parameter pack (clang 19 also warns at the pack's
// definition that `nonnull` applies only to pointers). spec/sema.md §4.5 "Parameter attributes",
// WP-2476.
//@ expect: ir
//@ std: c++20
//@ args: -Wno-ignored-attributes
template <class T, int N>
int f(T *p __attribute__((nonnull)), T *q __attribute__((noescape)),
      T *r __attribute__((align_value(N))), T *s __attribute__((annotate("x", N))),
      T u __attribute__((maybe_undef)), T w __attribute__((aligned(N))),
      T *const v __attribute__((pass_object_size(1)))) {
  return *p + *q + *r + *s + u + *v + w;
}

template <class T> struct S {
  T m(T *p __attribute__((nonnull)), T *q __attribute__((align_value(8)))) { return *p + *q; }
  template <int N> T n(T *p __attribute__((align_value(N)))) { return *p; }
};

template <class... T> int g(T __attribute__((nonnull)) ...p) { return (*p + ...); }

int use(int *a, int b) {
  int x = f<int, 32>(a, a, a, a, b, b, a);
  x += S<int>().m(a, a) + S<int>().n<64>(a);
  x += g(a, a);
  auto l = [](auto *p __attribute__((nonnull, align_value(16)))) { return *p; };
  return x + l(a);
}
