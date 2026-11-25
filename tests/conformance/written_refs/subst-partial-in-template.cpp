// Substitution keys (spec/ast.md §18.14.1, §18.14.10): a match triggered
// inside a function template's definition. `can_swap<n::S>` names no
// template parameter, so it is not dependent and the class is
// instantiated at the definition ([temp.res.general]/6, [temp.inst]/2),
// though `body` is never instantiated: the key, its `swap` row with the
// associated set, and its root, all resolved.
//@ written-refs: subst-partial-in-template.refs
//@ args: -fkiln-written-ref-keys
//@ std: c++26
namespace n {
struct S {};
void swap(S &, S &);
} // namespace n
template <class T> T &ref();
template <class T, class = void> struct can_swap {
  static const bool value = false;
};
template <class T> struct can_swap<T, decltype(swap(ref<T>(), ref<T>()))> {
  static const bool value = true;
};
template <class T> bool body(T) { return can_swap<n::S>::value; }
