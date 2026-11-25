// Substitution keys (spec/ast.md §18.14.10): a pack expansion among a
// partial specialization's written arguments has one root slot, which
// lists the pack's elements in order ([temp.variadic]/7,
// [temp.deduct.type]/9); an empty expansion prints `<empty>`. `R<int, A, B>`
// and `R<int, B, A>` bind the elements the other way round: two keys,
// two roots.
//@ written-refs: subst-partial-root-pack.refs
//@ args: -fkiln-written-ref-keys
//@ std: c++26
struct A {};
struct B {};
template <class T, class... Ts> struct R {
  static const int v = 0;
};
template <class... Ts> struct R<int, Ts...> {
  static const int v = sizeof...(Ts);
};
static_assert(R<int, A, B>::v == 2);
static_assert(R<int, B, A>::v == 2);
static_assert(R<int>::v == 0);
