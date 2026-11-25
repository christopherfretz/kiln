// Substitution keys (spec/ast.md §18.14.1, §18.14.3): a constrained
// partial specialization matches only when the deduced arguments satisfy
// its associated constraints ([temp.spec.partial.match]/2,
// [temp.constr.decl]/3). `K<int>` satisfies `Small` and gets a key;
// `K<double>` deduces and substitutes but does not satisfy it: no key.
// The requires-clause's own names (`Small`, `T`) are checked outside the
// key's scope: they have no `key=` line (their satisfaction is not
// keyed in this stage).
//@ written-refs: subst-partial-constrained.refs
//@ args: -fkiln-written-ref-keys
//@ std: c++26
template <class T> concept Small = sizeof(T) <= 4;
template <class T> T make();
template <class T, class = void> struct K {
  static const int v = 0;
};
template <class T>
  requires Small<T>
struct K<T, decltype(void(make<T>()))> {
  static const int v = 1;
};
static_assert(K<int>::v == 1);
static_assert(K<double>::v == 0);
