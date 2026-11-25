// Substitution keys off (spec/ast.md §18.14.7): subst-partial-adl.cpp's
// program with the written-reference table on and the keys off. The
// partial specialization still matches `can_swap<n::S>`
// ([temp.spec.partial.match]/2) and its call `swap` is still looked up
// at the substitution ([temp.dep.candidate]/1), but nothing of the match
// is recorded: no `key=` line and no `keys` line.
//@ written-refs: subst-partial-off.refs
//@ args: -fkiln-written-refs
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
