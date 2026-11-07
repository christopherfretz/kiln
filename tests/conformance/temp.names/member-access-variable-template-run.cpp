// [temp.names]/3, [expr.ref]/6.1: a class member access may name a
// static data member template, with or without `template`, through `.`
// or `->`, of the class or a base; `x.v<args>` names the specialization,
// a static data member (a sender library's `cpo.signature<Sig>` query
// signatures; WP-789). Dependent arguments or objects form it again at
// instantiation.
//@ expect: run
//@ std: c++20
//@ stdout: 8 4 14 12 12 14
extern "C" int printf(const char *, ...);
template <class T> struct sig { using type = T; int n = sizeof(T *); };
struct cpo_t {
  template <class Sig> static constexpr sig<Sig> signature{};
  template <int N> static constexpr int twice = 2 * N;
};
inline constexpr cpo_t cpo{};
struct D : cpo_t {};

template <class T> int dep(const cpo_t &c) { return c.signature<T>.n + c.template twice<sizeof(T)>; }
template <class C> int depobj(C *p) { return p->template twice<5> + p->cpo_t::template twice<1>; }
template <class T> int nondep() { return cpo.twice<3> + (int)sizeof(typename decltype(cpo.signature<T>)::type); }

int main() {
  constexpr auto q = cpo.signature<int() noexcept>;
  using T = decltype(q)::type;
  const cpo_t *p = &cpo;
  D d;
  static_assert(cpo.twice<4> == 8);
  printf("%d %d %d %d %d %d\n", (int)sizeof(T *), p->twice<2>, d.twice<7>, dep<short>(cpo), depobj(&d), nondep<long>());
}
