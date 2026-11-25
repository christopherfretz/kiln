// Substitution keys (spec/ast.md §18.14.1, §18.14.10): matches that do not
// complete get no key. `M<int, int>`: `T` is deduced from the first
// argument, the second is a non-deduced context ([temp.deduct.type]/5.1),
// and the substituted `int *` is not the specialization's `int`
// ([temp.deduct.type]/1): the partial specialization does not match
// ([temp.spec.partial.match]/2). `N<int, int>`: substituting
// `typename T::type` with `T` = `int` fails ([temp.deduct.general]/8):
// no match either. Neither has a key.
//@ written-refs: subst-partial-nonmatch.refs
//@ args: -fkiln-written-ref-keys
//@ std: c++26
template <class T> struct id {
  using type = T;
};
template <class T, class U> struct M {
  static const int v = 0;
};
template <class T> struct M<T, typename id<T>::type *> {
  static const int v = 1;
};
static_assert(M<int, int>::v == 0);
template <class T, class U> struct N {
  static const int v = 0;
};
template <class T> struct N<T, typename T::type> {
  static const int v = 1;
};
static_assert(N<int, int>::v == 0);
