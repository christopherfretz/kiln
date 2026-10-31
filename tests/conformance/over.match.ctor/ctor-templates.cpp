// [over.match.ctor]/1, [temp.over]/1: constructor templates are
// candidates alongside the constructors, including for a source of the
// class itself or a derived class ([dcl.init.general]/16.6.2.1) and in
// the dependent copies of a class template's own members.
//@ expect: compile-ok
template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
struct P {
  constexpr P(int P::*) noexcept {}
  template <class T, class = typename enable_if<!same<T, int>>::type>
  P(T) = delete;
};
struct O { int v; };
constexpr bool operator<(O o, P) noexcept { return o.v < 0; }
static_assert(O{-1} < 0);
struct W {
  template <class T> W(T t) : v(sizeof(t)) {}
  int v;
};
W w = 'c';
W w2(3.0);
struct X { X() = default; X(const X&) = default; template <class T> X(T) {} };
X a; X b = a; X c(a);
struct S {
  int k;
  constexpr S() : k(0) {}
  constexpr S(const S&) : k(1) {}
  template <class T> constexpr S(T) : k(2) {}
};
struct D : S {};
constexpr D d;
constexpr S s0;
static_assert(S(d).k == 2);
static_assert(static_cast<S>(d).k == 2);
constexpr S s1 = d;
static_assert(s1.k == 2);
static_assert(S(s0).k == 1);
constexpr S s2 = s0;
static_assert(s2.k == 1);
template <class T, class U> struct conv { static constexpr bool value = true; };
template <class It, class = void> struct BI {
  BI() = default;
  BI(BI const&) = default;
  BI(BI&&) = default;
  template <class O, typename enable_if<conv<O, It>::value, int>::type = 0>
  BI(BI<O> const& o) {}
  BI& operator++() { return *this; }
  BI operator++(int) { BI tmp(*this); ++*this; return tmp; }
};
BI<int*> x; BI<int*> y = x++; BI<int*> z(y);
template <class It> struct C {
  C() = default;
  C(C const&) = default;
  C(C&&) = default;
  template <class O> C(C<O> const& o) {}
  C f1() { C tmp(*this); return tmp; }
  C f2() const { C tmp(*this); return tmp; }
  void f3(C& o) { C tmp(o); }
  void f4(C<It>& o) { C tmp(o); }
};
void use(C<int*>& a, const C<int*>& b) {
  a.f1(); b.f2(); a.f3(a); a.f4(a);
  C<long*> c(a);
}
