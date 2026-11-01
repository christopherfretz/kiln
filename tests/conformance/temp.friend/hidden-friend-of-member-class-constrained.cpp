// [temp.friend]/9, [temp.dep.type]/1, [temp.constr.decl]/3 (WP-272): a
// hidden friend defined in a member class of a class template (libc++'s
// `filter_view<V, P>::__iterator`) names the member class in its
// parameter types; its trailing requires-clause is checked with the
// enclosing specialization's arguments, also when the friend is found
// only by argument-dependent lookup inside a concept's requires-expression.
//@ expect: run
//@ stdout: 1 0
#include <cstdio>
template <class T> concept eq = requires(const T& a) { a == a; };
struct NoEq {};
template <class V> struct view {
  struct iterator {
    V v;
    friend constexpr bool operator==(iterator const& x, iterator const& y)
      requires eq<V>
    { return x.v == y.v; }
  };
};
static_assert(eq<view<int>::iterator>);
static_assert(!eq<view<NoEq>::iterator>);
int main() {
  view<int>::iterator a{1}, b{1}, c{2};
  std::printf("%d %d\n", a == b, a == c);
}
