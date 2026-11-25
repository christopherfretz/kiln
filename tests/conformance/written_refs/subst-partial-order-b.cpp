// Substitution keys (spec/ast.md §18.14.1, §18.14.10): the keyed table
// does not depend on the order of the matches or on how a use spells its
// arguments. This file and subst-partial-order-a.cpp are the same program
// but for `run`: there `swappable<a::X>` is first named directly; here it
// is first matched inside the instantiation of `user<N>` the class
// member access forces ([temp.inst]/2), its argument spelled through the
// alias `N`. A key holds the canonical arguments ([temp.alias]/2), its
// root is built from the canonical deduced arguments, and the keys print
// sorted: the goldens differ only in the lines of `run`, roots included.
//@ written-refs: subst-partial-order-b.refs
//@ args: -fkiln-written-ref-keys
//@ std: c++26
namespace a {
struct X {};
void swap(X &, X &);
struct Y {};
void swap(Y &, Y &);
} // namespace a
template <class...> using void_t = void;
template <class T> T &ref();
template <class T, class = void> struct swappable {
  static const int v = 0;
};
template <class T> struct swappable<T, void_t<decltype(swap(ref<T>(), ref<T>()))>> {
  static const int v = 1;
};
template <class T> struct user {
  static const int v = swappable<T>::v;
};
using N = a::X;
int run() { return swappable<a::Y>::v + user<N>::v + swappable<N>::v; }
