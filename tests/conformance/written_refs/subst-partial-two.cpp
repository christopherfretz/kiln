// Substitution keys (spec/ast.md §18.14.1, §18.14.3): two partial
// specializations both match `W<P, int *>` ([temp.spec.partial.match]/1.3);
// the second is more specialized and is used
// ([temp.spec.partial.order]/1, [temp.func.order]). Each match is a key
// of its own, with its rows and its root; the partial ordering itself
// substitutes and deduces again and records nothing.
//@ written-refs: subst-partial-two.refs
//@ args: -fkiln-written-ref-keys
//@ std: c++26
struct P {};
template <class T> void touch(T);
template <class T, class U, class = void> struct W {
  static const int v = 0;
};
template <class T, class U> struct W<T, U *, decltype(touch(T()))> {
  static const int v = 1;
};
template <class T> struct W<T, int *, decltype(touch(T()))> {
  static const int v = 2;
};
static_assert(W<P, int *>::v == 2);
static_assert(W<P, long *>::v == 1);
