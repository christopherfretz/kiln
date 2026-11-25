// Substitution keys (spec/ast.md §18.14.1, §18.14.10): a partial
// specialization's argument list with no dependent name. `P<int *>`
// matches `P<T *>` ([temp.spec.partial.match]/2): a key without rows
// (`rows=0`) and its root, `root[0]=int *`. `P<char>` does not match:
// deduction fails ([temp.deduct.type]/1), no key.
//@ written-refs: subst-partial-no-dependent.refs
//@ args: -fkiln-written-ref-keys
//@ std: c++26
template <class T> struct P {
  static const int v = 0;
};
template <class T> struct P<T *> {
  static const int v = 1;
};
static_assert(P<int *>::v == 1);
static_assert(P<char>::v == 0);
