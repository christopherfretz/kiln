// Substitution keys (spec/ast.md §18.14.2, §18.14.10): a generic lambda in
// a partial specialization's argument list ([expr.prim.lambda.closure]/1,
// [temp.spec.partial.match]/2). The match substitutes `T` into the
// lambda's body, but `x.f` names a member of the lambda's own `auto`
// parameter, which stays dependent ([temp.dep.expr]/3): its keyed row is
// `-> (dependent)`. The closure type of the substituted lambda is
// reachable from the key's root.
//@ written-refs: subst-partial-generic-lambda.refs
//@ args: -fkiln-written-ref-keys
//@ std: c++26
template <class T, class = void> struct G {
  static const int v = 0;
};
template <class T>
struct G<T, decltype(void(T()), void([](auto x) { return x.f(T()); }))> {
  static const int v = 1;
};
struct H {};
static_assert(G<H>::v == 1);
