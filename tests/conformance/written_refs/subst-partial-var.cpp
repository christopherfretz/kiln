// Substitution keys (spec/ast.md §18.14.1): a variable template partial
// specialization ([temp.spec.partial.general]/1) matches as a class
// template's does ([temp.spec.partial.match]/2): `key=partial:` names the
// variable template's partial specialization, and the member call `f` in
// its arguments has its keyed row.
//@ written-refs: subst-partial-var.refs
//@ args: -fkiln-written-ref-keys
//@ std: c++26
template <class T> T make();
template <class T, class = void> constexpr int vt = 0;
template <class T> constexpr int vt<T, decltype(make<T>().f())> = 1;
struct F {
  void f();
};
static_assert(vt<F> == 1);
static_assert(vt<int> == 0);
