// [temp.deduct.partial]/12, [temp.deduct.type]/9: in partial ordering, a
// pack expansion of the parameter template that matches no element of
// the other template's argument list deduces each pack it names as
// empty, so the pack has a value and does not fail the deduction. The
// overload with the empty list is more specialized (Boost.Mp11
// `mp_for_each_impl(mp_list<T...>, F&&)` against `(mp_list<>, F&&)`,
// `tuple_for_each_impl` with `integer_sequence<size_t, J...>`).
//@ expect: run
//@ stdout: 2 2 2 1 1 1 4 3
#include <cstddef>
#include <cstdio>

template <class... T> struct L {};
template <class T, T... I> struct Seq {};
template <template <class> class... TT> struct TL {};
template <class> struct Box {};

template <class... T, class F> int f(L<T...>, F&&) { return 1; }
template <class F> int f(L<>, F&&) { return 2; }
template <class Tp, std::size_t... J, class F> int g(Tp&&, Seq<std::size_t, J...>, F&&) { return 1; }
template <class Tp, class F> int g(Tp&&, Seq<std::size_t>, F&&) { return 2; }
template <template <class> class... TT> int h(TL<TT...>) { return 1; }
inline int h(TL<>) { return 2; }
// A pattern naming a pack and a parameter that is not one.
template <class U, class... T> int k(U, L<Box<T>...>) { return 1; }
template <class U> int k(U*, L<>) { return 4; }
template <class... T> int m(L<int, T...>) { return 1; }
template <class... T> int m(L<int>) { return 3; }

int main() {
  int t = 0;
  std::printf("%d %d %d %d %d %d %d %d\n", f(L<>(), 0), g(t, Seq<std::size_t>(), 0), h(TL<>()),
              f(L<int>(), 0), g(t, Seq<std::size_t, 1>(), 0), h(TL<Box>()), k(&t, L<>()),
              m(L<int>()));
}
