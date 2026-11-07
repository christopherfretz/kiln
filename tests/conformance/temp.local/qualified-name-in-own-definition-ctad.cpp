// [temp.local]/1, [dcl.type.class.deduct]/1: inside a class template's own
// definition, its name qualified by a namespace (`::n::q`, `n::q`) is the
// template, never the injected-class-name, so `::n::q{t, 1}` deduces its
// arguments as it would outside (mp-units' delegating constructor
// `quantity(::mp_units::quantity{v, R2{}})`). Unqualified, the name stays
// the injected-class-name.
// expected-no-diagnostics
namespace n {
template<class T> struct q {
  T v;
  constexpr q(T t, int) : v(t) {}
  constexpr q(T t, long) : q(::n::q{t, 1}) {}
  template<class U> static constexpr auto f(U u) { return ::n::q{u, 1}; }
  template<class U> static constexpr auto g(U u) { return n::q(u, 1); }
  template<class U> static constexpr auto h(U u) { return q(u, 1); }
  template<class U> static constexpr auto k(U u) {
    ::n::q x{u, 1};
    return x;
  }
};
}
template<class, class> constexpr bool same = false;
template<class T> constexpr bool same<T, T> = true;
constexpr n::q<double> a(2.5, 3L);
static_assert(a.v == 2.5);
static_assert(same<decltype(n::q<int>::f(4.5)), n::q<double>>);
static_assert(n::q<int>::f(4.5).v == 4.5);
static_assert(same<decltype(n::q<int>::g(5L)), n::q<long>>);
static_assert(same<decltype(n::q<int>::h(6.5)), n::q<int>>);
static_assert(n::q<int>::h(6.5).v == 6);
static_assert(same<decltype(n::q<int>::k('c')), n::q<char>>);
