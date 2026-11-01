// [temp.friend]/9, [temp.dep.type]/1, [temp.constr.decl]/3 (WP-272):
// variants of a constrained hidden friend of a member class of a class
// template: a sentinel compared with an iterator (libc++'s
// `filter_view::__sentinel`), member class templates `iterator<Const>` /
// `sentinel<Const>` (`join_view`, `elements_view`), two levels of member
// classes, constraints naming members of the current instantiation, a
// constrained member function instead of a friend, a friend template, and
// requires-expressions naming the friend's parameters.
//@ expect: run
//@ stdout: 1 1 1 1 1
#include <cstdio>
template <class T> concept eq = requires(const T& a) { a == a; };
template <class S, class I> concept sentinel_for = requires(const I& i, const S& s) {
  { i == s } -> eq;
};
struct NoEq {};

template <class V> struct filter {
  using value_type = V;
  struct iterator {
    V v;
    friend constexpr bool operator==(iterator const& x, iterator const& y)
      requires eq<typename filter::value_type>
    { return x.v == y.v; }
  };
  struct sentinel {
    V end;
    friend constexpr bool operator==(iterator const& x, sentinel const& y)
      requires eq<V> && (sizeof(iterator) == sizeof(V))
    { return x.v == y.end; }
  };
};
static_assert(sentinel_for<filter<int>::sentinel, filter<int>::iterator>);
static_assert(!sentinel_for<filter<NoEq>::sentinel, filter<NoEq>::iterator>);

template <class V> struct join {
  template <bool Const> struct sentinel;
  template <bool Const> struct iterator {
    V v;
    friend constexpr bool operator==(const iterator& x, const iterator& y)
      requires eq<V> && Const
    { return x.v == y.v; }
    friend constexpr bool operator==(const iterator& x, const sentinel<Const>& y)
      requires eq<V>
    { return x.v == y.end; }
  };
  template <bool Const> struct sentinel { V end; };
};
static_assert(eq<join<int>::iterator<true>>);
static_assert(!eq<join<int>::iterator<false>>);
static_assert(!eq<join<NoEq>::iterator<true>>);
static_assert(sentinel_for<join<int>::sentinel<false>, join<int>::iterator<false>>);
static_assert(!sentinel_for<join<NoEq>::sentinel<true>, join<NoEq>::iterator<true>>);

template <class T> struct outer {
  struct mid {
    template <class U> struct inner {
      T t;
      U u;
      friend constexpr bool operator==(const inner& x, const inner& y)
        requires eq<T> && eq<U> && eq<typename outer::mid>
      { return x.t == y.t && x.u == y.u; }
    };
    friend constexpr bool operator==(mid, mid) { return true; }
  };
};
static_assert(eq<outer<int>::mid::inner<long>>);
static_assert(!eq<outer<int>::mid::inner<NoEq>>);
static_assert(!eq<outer<NoEq>::mid::inner<int>>);

template <class V> struct member_fn {
  struct iterator {
    V v;
    constexpr bool same(const iterator& y) const
      requires eq<V>
    { return v == y.v; }
  };
};
template <class I> concept has_same = requires(const I& i) { i.same(i); };
static_assert(has_same<member_fn<int>::iterator>);
static_assert(!has_same<member_fn<NoEq>::iterator>);

// A friend template, and requires-expressions naming the friend's
// parameters and the current instantiation.
template <class V> struct params {
  struct iterator {
    V v;
    template <class U>
    friend constexpr bool cmp(iterator const& x, U const& u)
      requires eq<V> && eq<U>
    { return x.v == u; }
    friend constexpr bool operator==(iterator const& x, iterator const& y)
      requires requires { x.v == y.v; }
    { return x.v == y.v; }
    friend constexpr bool operator<(iterator const& x, iterator const& y)
      requires requires { x.v < y.v; typename params::iterator; }
    { return x.v < y.v; }
  };
};
template <class I, class U> concept cmpable = requires(I i, U u) { cmp(i, u); };
template <class T> concept lt = requires(const T& a) { a < a; };
static_assert(cmpable<params<int>::iterator, long>);
static_assert(!cmpable<params<NoEq>::iterator, long>);
static_assert(!cmpable<params<int>::iterator, NoEq>);
static_assert(eq<params<int>::iterator> && lt<params<int>::iterator>);
static_assert(!eq<params<NoEq>::iterator> && !lt<params<NoEq>::iterator>);

int main() {
  filter<int>::iterator i{3};
  filter<int>::sentinel s{3};
  join<int>::iterator<true> j{1}, k{1};
  join<int>::sentinel<true> js{1};
  outer<int>::mid::inner<long> m{1, 2};
  member_fn<int>::iterator f{5};
  std::printf("%d %d %d %d %d\n", i == i && i == s, j == k, j == js, m == m, f.same(f));
}
