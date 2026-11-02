// [dcl.type.pack.index] (C++26, P2662), [expr.prim.id.qual]/1: a
// pack-index-specifier is a computed-type-specifier, so `Ts...[I]` may be
// the first component of a nested-name-specifier (a type, a member typedef,
// a member template, a static member) and a class-or-decltype
// ([class.derived.general]/1); the qualifier is dependent until the pack is
// substituted.
//@ requires: clang>=22
// (clang 19 crashes on this file.)
struct A {
  using type = int;
  static constexpr int v = 1;
  struct In { using t = char; };
  template <class U> using ptr = U *;
};
struct B { using type = long; static constexpr int v = 2; };

template <class... Ts> struct L {
  using first = typename Ts...[0]::type;
  using second = Ts...[1]::type;
  using inner = typename Ts...[0]::In::t;
  using tmpl = typename Ts...[0]::template ptr<int>;
  static constexpr int sum = Ts...[0]::v + Ts...[1]::v;
};
static_assert(__is_same(L<A, B>::first, int));
static_assert(__is_same(L<A, B>::second, long));
static_assert(__is_same(L<A, B>::inner, char));
static_assert(__is_same(L<A, B>::tmpl, int *));
static_assert(L<A, B>::sum == 3);

template <class... Ts> struct D : Ts...[0] {};
template <class... Ts> struct E : Ts...[1]::In {};
static_assert(D<A, B>::v == 1);
static_assert(__is_same(E<B, A>::t, char));

template <int I, class... Ts> constexpr int at() {
  typename Ts...[I]::type x = Ts...[I]::v;
  return x;
}
static_assert(at<0, B, A>() == 2);
static_assert(at<1, B, A>() == 1);
