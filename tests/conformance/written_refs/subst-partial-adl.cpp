// Substitution keys (spec/ast.md §18.14.1, §18.14.10): a class template
// partial specialization matches a specialization when its arguments
// can be deduced from the specialization's ([temp.spec.partial.match]/2,
// [temp.deduct.type]/1), the deduced arguments substituted into the
// partial specialization's argument list giving the specialization's
// ([temp.deduct.type]/1, [temp.deduct]/5). The unqualified call `swap`
// in that list is looked up when the list is substituted
// ([temp.dep.candidate]/1, [basic.lookup.argdep]/2): its keyed row names
// n::swap with the associated namespaces and classes, under the key of
// the match `can_swap<n::S, void>`, and the key's root holds the
// substituted list (`root[1]`). For n::U the substitution fails: no key.
// A partial specialization's argument list is not part of a
// specialization's definition: its entries have no `spec=` line.
//@ written-refs: subst-partial-adl.refs
//@ args: -fkiln-written-ref-keys
//@ std: c++26
namespace n {
struct S {};
void swap(S &, S &);
struct U {};
} // namespace n
template <class T> T &&declval();
template <class T, class = void> struct can_swap {
  static constexpr bool value = false;
};
template <class T>
struct can_swap<T, decltype(swap(declval<T &>(), declval<T &>()))> {
  static constexpr bool value = true;
};
static_assert(can_swap<n::S>::value);
static_assert(!can_swap<n::U>::value);
