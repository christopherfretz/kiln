// Substitution keys (spec/ast.md §18.13, §18.14.10): subst-partial-oper.cpp
// with the operator declared in the global namespace before the partial
// specialization. Unqualified lookup at the definition finds it
// ([over.match.oper]/3.2, [temp.dep.candidate]/1); the match of
// `shl<a::X>` ([temp.spec.partial.match]/2) is the same, so is its key,
// and the `<<` token's keyed row names ::operator<<.
//@ written-refs: subst-partial-oper-global.refs
//@ args: -fkiln-written-ref-keys
//@ std: c++26
namespace a {
struct X {};
} // namespace a
int operator<<(a::X, a::X);
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
