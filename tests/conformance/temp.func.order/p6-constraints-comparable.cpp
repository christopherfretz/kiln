// [temp.func.order]/6.1-6.2.2: constraints break a tie between equally
// specialized templates only when their template-parameter-lists and
// function parameter lists have the same length and the corresponding
// template parameters and function parameter types (top-level cv
// ignored) are the same. Otherwise neither is more specialized, and the
// later tie-breakers decide; here [over.match.best]/2.11 prefers the
// user-defined deduction guide over the constrained constructor guide
// (the shape of libc++'s `zoned_time` guides).
//@ strict-diags
template <class D> struct tp { D d; };
template <class D, class P = int> struct zt {
  zt(const tp<D> &) requires true {}
};
template <class D> zt(tp<D>) -> zt<D>;
template <class T> zt(T &&) -> zt<int, T>;
zt z{tp<long>{}};
static_assert(__is_same(decltype(z), zt<long, int>));

// Same lists, differing only in constraints: the more constrained wins.
template <class T> concept Big = sizeof(T) > 1;
template <class T> constexpr int f(T) { return 1; }
template <class T> requires Big<T> constexpr int f(T) { return 2; }
static_assert(f('c') == 1 && f(0L) == 2);
// Top-level cv on a parameter is not part of its type.
template <class T> constexpr int g(const T) { return 1; }
template <class T> requires Big<T> constexpr int g(T) { return 2; }
static_assert(g(0L) == 2);
