// Substitution keys (spec/ast.md §18.14.1, §18.14.4): qualified names in a
// partial specialization's argument list. `typename T::type` and
// `T::value` are in non-deduced contexts ([temp.deduct.type]/5.1,
// /5.3): the match
// substitutes the deduced `T` into them and compares the result with the
// specialization's arguments ([temp.spec.partial.match]/2,
// [temp.deduct.type]/1). Two specializations bind them to different
// members: each entry has two `key=` lines, one per key, in key order.
//@ written-refs: subst-partial-qualified.refs
//@ args: -fkiln-written-ref-keys
//@ std: c++26
template <class T, class = void, class = void> struct Q {
  static const int v = 0;
};
template <class T> struct Q<T, typename T::type, decltype(void(T::value))> {
  static const int v = 1;
};
struct A {
  using type = void;
  static constexpr int value = 0;
};
struct B {
  using type = void;
  static constexpr int value = 0;
};
static_assert(Q<A>::v == 1);
static_assert(Q<B>::v == 1);
