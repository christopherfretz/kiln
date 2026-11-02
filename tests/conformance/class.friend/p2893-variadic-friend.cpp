//@ expect: compile-ok
//@ requires: clang>=22
//@ strict-diags
// [class.friend]/3 (C++26 P2893): a friend-type-specifier-list befriends
// each of its types, and `friend Ts...;` is a pack expansion
// ([temp.variadic]/5.6) befriending each element of the instantiation;
// a non-class element is ignored ([class.friend]/3). Elaborated
// specifiers in the list declare their class in the enclosing namespace
// ([dcl.type.elab]/3).
template <class... Ts> class C {
  friend Ts...;
  static constexpr int secret = 42;
};
struct A { static constexpr int get() { return C<int, A>::secret; } };
static_assert(A::get() == 42);
C<> none; // an empty expansion befriends nothing

struct P { struct inner { static constexpr int get(); }; };
class D {
  friend A, struct E, P::inner, int;
  static constexpr int z = 7;
};
struct E { static constexpr int get() { return D::z; } };
constexpr int P::inner::get() { return D::z; }
static_assert(E::get() == 7 && P::inner::get() == 7);

// A list in a class template: a plain type, an expansion, an expansion of
// a typename-specifier and an elaborated specifier.
struct Q { struct inner { static constexpr int get(); }; };
template <class... Ts> class F {
  friend class A, Ts..., typename Ts::inner..., struct G;
  static constexpr int v = 3;
};
struct G { static constexpr int get() { return F<P, Q>::v; } };
constexpr int Q::inner::get() { return F<P, Q>::v; }
struct H {
  struct inner {};
  static constexpr int get() { return F<H>::v; }
};
static_assert(G::get() == 3 && Q::inner::get() == 3 && H::get() == 3);

// Friends from two packs of nested templates; befriending a dependent
// specialization.
template <class... Ts> struct Outer {
  template <class... Us> class In {
    friend Ts..., Us...;
    static constexpr int w = 5;
  };
};
struct I { static constexpr int get() { return Outer<I>::In<A>::w; } };
struct J { static constexpr int get() { return Outer<A>::In<J, int>::w; } };
static_assert(I::get() == 5 && J::get() == 5);
template <class T> struct K { static constexpr int get() { return C<K<T>, T>::secret; } };
static_assert(K<int>::get() == 42);
