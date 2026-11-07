// [temp.variadic]/5, [temp.dep.type]/1: inside a class template, a
// template-id of the template itself whose first argument is a pack
// expansion of template template parameters (googletest's
// `Templates<Tail_...>`) is a dependent specialization; it is not
// checked against the parameters as a current-instantiation candidate.
//@ expect: run
//@ stdout: 2 1
#include <cstdio>

template <template <class> class H, template <class> class... T>
struct Ts {
  using Tail = Ts<T...>;
  static constexpr int size = 1 + Tail::size;
};
template <template <class> class H>
struct Ts<H> {
  static constexpr int size = 1;
};

template <class> struct V {};
template <class> struct W {};

int main() { std::printf("%d %d\n", Ts<V, W>::size, Ts<W>::size); }
