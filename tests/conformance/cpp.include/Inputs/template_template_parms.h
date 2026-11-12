// Template template parameters (WP-1342): with a default, as a pack, as
// a partial specialization's argument and parameter, named with
// arguments in a base, a default and a member, passed to a function
// template, and deducing class template arguments (`C(a)`).
#ifndef TEMPLATE_TEMPLATE_PARMS_H
#define TEMPLATE_TEMPLATE_PARMS_H
namespace ttp {
template <class T> struct box { T v; };
template <class T, class U = int> struct duo { T a; U b; };
template <template <class...> class F, class... A> struct lazy : F<A...> {};
template <template <class...> class T, class... A, class = T<A...>>
constexpr bool valid(int) { return true; }
template <template <class...> class, class...>
constexpr bool valid(...) { return false; }
template <class T> struct first;
template <template <class, class...> class S, class T, class... A>
struct first<S<T, A...>> { using type = T; };
template <class T, template <class...> class Tmpl>
constexpr bool is_spec = false;
template <template <class...> class Tmpl, class... A>
constexpr bool is_spec<Tmpl<A...>, Tmpl> = true;
template <class T, template <class> class W = box> struct wrap { W<T> w; };
template <template <class> class... Ws> struct many {
  static constexpr int n = sizeof...(Ws);
};
template <template <class> class W, class T> constexpr int wsize() {
  return sizeof(W<T>);
}
template <class T> struct meta {
  template <class U> struct apply { duo<T, U> d; };
};
template <class T, template <class> class Q> struct with { Q<T> q; };
template <class T> using meta_of = with<long, meta<T>::template apply>;
template <class T> struct cell { T v; cell(T t) : v(t) {} };
template <class T> T &&decl();
template <template <class...> class C, class A>
using deduce = decltype(C(decl<A>()));
template <template <class...> class C, class A> auto make(A a) { return C(a); }
} // namespace ttp
#endif
