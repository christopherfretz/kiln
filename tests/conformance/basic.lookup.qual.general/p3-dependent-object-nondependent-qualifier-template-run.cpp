// [basic.lookup.qual.general]/2-3, [temp.names]/7.3.2: in `p->N::f<U>()`
// with a type-dependent `p`, `N` is a member-qualified name whose lookup
// context is dependent, so it is not looked up there and falls back to
// unqualified lookup, which finds the class `N`. `f` is then a qualified
// name whose lookup context is the non-dependent `N`: it is looked up
// where it appears, finds a member template, and the `<` starts a
// template argument list without `template`. A data member found the same
// way keeps `<` a less-than. Found running GCC's C++ testsuite (WP-1530).
// GCC 16.1 agrees; clang 22 asks for 'template' (it keeps the pre-P1787
// rule), so the text decides.
//@ expect: run
//@ oracle: gcc
//@ skip(clang): clang 22 requires 'template' after a non-dependent qualifier of a dependent object
//@ strict-diags
struct impl {
  template <class T> static T create() { return T(); }
  template <class T> int g() { return int(sizeof(T)); }
  int h = 5;
};
template <class T, class U, class = decltype(impl::create<T>()->impl::create<U>())>
struct tester { int v = 1; };

template <class T, class U, class = decltype(impl::create<T>()->impl::create<U>())>
int test() { return 2; }

template <class T> int k(T t, T *p) {
  int a = t.impl::g<char>();      // 1
  int b = p->impl::g<short>();    // 2
  bool c = t.impl::h < 7;         // a comparison: h is no template
  return a + b + (c ? 10 : 0);
}

int main() {
  tester<impl *, int> ti;
  impl i;
  return ti.v + test<impl *, int>() + k(i, &i) == 16 ? 0 : 1;
}
