// Substitution keys (spec/ast.md §18.9.1, §18.14.1): a match inside the
// substitution of a default template argument. `holder<n::S>` takes its
// default argument `typename trait<T>::type` ([temp.arg.general]/9),
// which instantiates `trait<n::S>`, which the partial specialization
// matches ([temp.spec.partial.match]/2). The partial specialization's key
// has its rows (`pick`, `val`); the default argument's entries keep their
// `spec=` lines; no row of one is under the other.
//@ written-refs: subst-partial-nested.refs
//@ args: -fkiln-written-ref-keys
//@ std: c++26
namespace n {
struct S {};
int pick(S);
} // namespace n
template <class T> T val();
template <class T, class = void> struct trait {
  using type = long;
};
template <class T> struct trait<T, decltype(void(pick(val<T>())))> {
  using type = int;
};
template <class T, class U = typename trait<T>::type> struct holder {
  U u;
};
holder<n::S> h;
static_assert(sizeof(h.u) == sizeof(int));
