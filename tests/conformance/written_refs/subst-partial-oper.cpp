// Substitution keys (spec/ast.md §18.13, §18.14.10): an operator in a
// partial specialization's argument list. When the match substitutes
// `val<a::X>() << val<a::X>()` ([temp.spec.partial.match]/2,
// [temp.deduct]/5), the operator's function is found by argument-dependent
// lookup ([over.match.oper]/3, [basic.lookup.argdep]/2): the `<<` token's
// keyed row names a::operator<< with its lookup facts, though no written
// name selects it, and the key's `root[1]` holds the operator call.
// subst-partial-oper-global.cpp declares the operator at namespace scope
// before the template: the same key, another `<<` row.
//@ written-refs: subst-partial-oper.refs
//@ args: -fkiln-written-ref-keys
//@ std: c++26
namespace a {
struct X {};
int operator<<(X, X);
} // namespace a
template <class...> using void_t = void;
template <class T> T val();
template <class T, class = void> struct shl {
  static const int v = 0;
};
template <class T> struct shl<T, void_t<decltype(val<T>() << val<T>())>> {
  static const int v = 1;
};
int u = shl<a::X>::v;
static_assert(shl<a::X>::v == 1);
