export module imgmn;
export namespace mf {
struct true_type { static const bool value = true; };
struct false_type { static const bool value = false; };
struct impl {
  template <class T> static T &test(int);
  template <class T> static false_type test(...);
};
template <class T> struct is_ref : decltype(impl::test<T>(0)) {};
template <class T> struct is_ref2 {
  static const bool value = !__is_same(decltype(impl::test<T>(0)), false_type);
};
template <class T> struct holder {
  template <class U> static int pick(U u) { return int(u) + 1; }
  static int pick(...) { return 0; }
  static int go(T t) { return pick<T>(t) + holder::pick(t); }
  int inst(T t) { return pick(t) * 2; }
};
template <class F> struct inv {
  template <class X> static decltype(X()(1)) try_call(int);
  template <class X> static false_type try_call(...);
  using R = decltype(try_call<F>(0));
};
}
