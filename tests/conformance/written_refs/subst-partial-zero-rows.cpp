// Substitution keys (spec/ast.md §18.14.1): a completed match is a key
// whether or not a name was looked up. `X<tuple<>, void()>` deduces an
// empty pack ([temp.deduct.type]/9), so `typename Ts::type...` expands to
// nothing ([temp.variadic]/7): its key has no row (`rows=0`). The match of
// `X<tuple<n::S>, void(int)>` looks `type` up in n::S: a row.
//@ written-refs: subst-partial-zero-rows.refs
//@ args: -fkiln-written-ref-keys
//@ std: c++26
template <class...> struct tuple {};
namespace n {
struct S {
  using type = int;
};
} // namespace n
template <class A, class B> struct X {
  static const int v = 0;
};
template <class... Ts> struct X<tuple<Ts...>, void(typename Ts::type...)> {
  static const int v = 1;
};
static_assert(X<tuple<>, void()>::v == 1);
static_assert(X<tuple<n::S>, void(int)>::v == 1);
