// Mangling differential: dependent template-specialization types in
// function template signatures ([temp.dep.type]; Itanium ABI §5.1.5
// <unscoped-template-name>, nested <template-prefix>, <template-args>,
// template template parameters). WP-117: these used to mangle as
// `u7unknown`.
namespace std {
template <class E> struct initializer_list { const E *b; decltype(sizeof 0) n; };
template <class E> struct W {};
}
namespace N {
template <class E> struct W {};
namespace M { template <class E, class F> struct P {}; }
}
template <class E> struct W {};
template <class E> struct Out { template <class F> struct In {}; };
struct V {
  template <class T> V(std::initializer_list<T>) {}
  template <class T> V(std::W<T>, int) {}
  template <class T> V(N::W<T>, long) {}
  template <class T> V(W<T>, char) {}
};
template <class T> void f(std::initializer_list<T>) {}
template <class T> void g(std::W<T>) {}
template <class T> void h(W<T>, W<T>, W<T *>) {}
template <class T, class U> void p(N::M::P<T, U>, N::M::P<U, T>, N::W<T>) {}
template <template <class> class TT, class T> void k(TT<T>, TT<int>) {}
template <template <class> class TT> void k2(TT<int>, TT<int>, TT<char>) {}
template <template <class> class TT, class T> void k3(T, TT<T>, TT<int>) {}
template <class A, template <class> class TT> void k4(TT<int>, TT<char>) {}
template <template <class> class X> struct H {};
template <template <class> class TT> void k5(H<TT>, TT<int>) {}
template <class T> void q(W<W<T>> *, const W<T> &) {}
void use() {
  V(std::initializer_list<int>{});
  V(std::W<int>{}, 1);
  V(N::W<int>{}, 1L);
  V(W<int>{}, 'c');
  f(std::initializer_list<int>{});
  g(std::W<int>{});
  h(W<int>{}, W<int>{}, W<int *>{});
  p(N::M::P<int, char>{}, N::M::P<char, int>{}, N::W<int>{});
  k(W<int>{}, W<int>{});
  k2(W<int>{}, W<int>{}, W<char>{});
  k3(1, W<int>{}, W<int>{});
  k4<long>(W<int>{}, W<char>{});
  k5(H<W>{}, W<int>{});
  W<W<long>> w;
  q(&w, W<long>{});
}
