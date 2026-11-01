// [class.base.init]/2: a mem-initializer-id may be a simple-template-id
// naming a base, also as a pack expansion ([temp.variadic]/5; the libc++
// `__tuple_leaf<_Indx, _Tp>(...)...` pattern). Using a class template
// specialization's constructor in a mem-initializer instantiates that
// constructor without disturbing the enclosing ctor-initializer (WP-141).
//@ expect: run
template <int I, class T> struct Leaf {
  T v;
  constexpr Leaf(T x) : v(x) {}
};
struct A : Leaf<0, int> {
  constexpr A() : Leaf<0, int>(3) {}
};
static_assert(A().v == 3);

template <class T> struct B : Leaf<1, T> {
  T w;
  constexpr B() : Leaf<1, T>(4), w(5) {}
};
static_assert(B<int>().v == 4 && B<int>().w == 5);

using L2 = Leaf<2, int>;
struct C : L2 {
  int w;
  constexpr C() : L2(6), w(7) {}
};
static_assert(C().v == 6 && C().w == 7);

template <int...> struct Idx {};
template <class I, class... Ts> struct Impl;
template <int... Is, class... Ts>
struct Impl<Idx<Is...>, Ts...> : Leaf<Is, Ts>... {
  template <class... Us>
  constexpr Impl(int, Us... u) : Leaf<Is, Ts>(u)... {}
};
constexpr Impl<Idx<0, 1>, int, long> x(0, 1, 2L);
static_assert(static_cast<const Leaf<0, int>&>(x).v == 1);
static_assert(static_cast<const Leaf<1, long>&>(x).v == 2);

int main() {
  Impl<Idx<0, 1>, int, long> y(0, 8, 9L);
  return static_cast<Leaf<0, int>&>(y).v + static_cast<Leaf<1, long>&>(y).v == 17 ? 0 : 1;
}
