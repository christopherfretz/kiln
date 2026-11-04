// [over.match.funcs]/4-5, [class.mfct.non.static]/2, [temp.arg.explicit]/4:
// a member function template-id called without an object expression
// (`get<0>()` found in a base, `B::get<0>()` qualified, also inside `B`
// itself) has the implicit object argument `*this`, so the implicit
// object parameters of `get()` and `get() const` rank the candidates as
// for `this->get<0>()` and as for a non-template name: the non-const one
// wins for a non-const `*this`, the const one for a const `*this`, the
// only (const) one is viable for a non-const `*this`, and a ref-qualified
// pair picks `&` (`*this` is an lvalue). Also in a member of a class
// template (a type-dependent `this`), through a lambda's captured
// `this`, and with a static member template in the set. WP-543.
//@ expect: run
//@ exit: 42
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };
struct B {
  template <int I> int &get() { return v; }
  template <int I> const int &get() const { return v; }
  template <int I> int only_const() const { return 2; }
  template <int I> static int s(int) { return 1; }
  template <int I> int s(long) { return 2; }
  template <int I> int r() & { return 1; }
  template <int I> int r() && { return 100; }
  int v = 1;
  // Inside the class itself.
  template <int I> int &g2() { return this->get<I>(); }
  template <int I> int &g3() { return get<I>(); }
  template <int I> int &g4() { return B::get<I>(); }
  int &g5() { return B::get<0>(); }
};
struct D : B {
  int &g() { return B::get<0>(); }
  int &h() { return this->get<0>(); }
  int &k() { return get<0>(); }
  int c1() const {
    static_assert(same<decltype(get<0>()), const int &>::value, "");
    static_assert(same<decltype(B::get<0>()), const int &>::value, "");
    return get<0>() + B::get<0>();
  }
  int c2() { return only_const<0>() + B::only_const<0>(); }
  int c3() { return s<0>(0) + s<0>(0L) + B::s<0>(0); }
  int c4() { return r<0>() + B::r<0>(); }
  int c5() {
    return [this] { return get<0>() + B::get<0>(); }();
  }
  template <int I> int &t() { return B::get<I>(); }
};
template <class T> struct S {
  template <int I> T &get() { return v; }
  template <int I> const T &get() const { return v; }
  T v = 1;
  T &f() { return get<0>(); }
  T &g() { return S::get<0>(); }
  T &h() { return S<T>::template get<0>(); }
};
template <class T> struct E : B {
  int &g() { return B::get<0>(); }
  int &k() { return get<0>(); }
};
static_assert(same<decltype(static_cast<D *>(nullptr)->g()), int &>::value, "");
int main() {
  D d;
  d.g() = 1; // non-const references: assignable
  d.k() = 1;
  int n = d.g() + d.h() + d.k();          // 3
  n += d.g2<0>() + d.g3<0>() + d.g4<0>(); // 6
  n += d.g5() + d.t<0>();                 // 8
  n += d.c1();                            // 10
  n += d.c2();                            // 14
  n += d.c3();                            // 18
  n += d.c4();                            // 20
  n += d.c5();                            // 22
  S<int> s;
  n += s.f() + s.g() + s.h();             // 25
  E<int> e;
  n += e.g() + e.k();                     // 27
  return n + 15;
}
