// Mangling differential (spec/abi.md §8.15, WP-818): in a constraint of
// a member of class template `X` (mangled uninstantiated), `X` and
// `X<P...>` for X's own parameters name the injected-class-name, which
// clang makes canonical: it mangles as the specialization and then takes
// a substitution of its own, so later back-references shift by one
// (libstdc++ `ranges::ref_view`'s constructor), also when `X` has
// non-type parameters or packs (`subrange<I, S, K>`'s constructor). A
// specialization of `X` for other arguments, or outside `X`, takes one.
template <class T, class U> concept different_from = !__is_same(T, U);
template <class T, class U> concept convertible_to = __is_convertible(T, U);
template <class T, class U> concept C = true;
template <class T> T&& declval();
template <class X> struct W {};
struct V {};
template <class T> struct W<T&> { W(V&); };

template <class R> struct ref_view {
  static void fun(R&);
  template <different_from<ref_view> T>
    requires convertible_to<T, R&> && requires { fun(declval<T>()); }
  ref_view(T&&) {}
};
template <class R> struct spelled {
  template <different_from<spelled<R>> T>
    requires convertible_to<T, R&> && convertible_to<T, W<T>>
  spelled(T&&) {}
};
template <class R> struct other {
  template <different_from<other<R*>> T>
    requires convertible_to<T, R&> && convertible_to<T, W<T>>
  other(T&&) {}
};
template <class R, class... P> struct X {
  template <class U> void g(X<U>*, U*, X<U>*) {}
  template <class U> requires C<U, X> && C<U, W<U>> void h(X*, U, X<R, P...>*) {}
  template <class U> requires C<U, X<R, P...>> && C<U, X<U>> && C<U, W<U>> void k(U, X<U>*) {}
};
template <class I, int K> struct Z {
  template <different_from<Z> T>
    requires convertible_to<T, I&> && convertible_to<T, W<T>>
  Z(T&&) {}
};
template <class I, int... K> struct Zp {
  template <different_from<Zp> T>
    requires convertible_to<T, I&> && convertible_to<T, W<T>>
  Zp(T&&) {}
};
template <class R> struct Y {
  template <class U> requires C<U, X<R>> && C<U, W<U>> void m(X<R>*) {}
};

void use() {
  V v;
  ref_view<V> r(v);
  spelled<V> s(v);
  other<V> o(v);
  Z<V, 1> z(v);
  Zp<V, 1, 2> zp(v);
  X<int>().g<char>(0, 0, 0);
  X<int, long> x;
  x.h(&x, 1, &x);
  X<int>().k<char>(1, 0);
  Y<int>().m<char>(0);
}
