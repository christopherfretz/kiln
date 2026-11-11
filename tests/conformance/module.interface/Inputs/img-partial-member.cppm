export module imgop;
export namespace op {
template <class T> struct F;
template <class R, class... A> struct F<R(A...)> {
  R (*p)(A...);
  F(R (*q)(A...));
  R call(A... a) const;
  static int count;
};
template <class R, class... A> F<R(A...)>::F(R (*q)(A...)) { p = q; }
template <class R, class... A> R F<R(A...)>::call(A... a) const { return p(a...); }
template <class R, class... A> int F<R(A...)>::count = sizeof...(A);
inline int twice(int x) { return 2 * x; }
inline int use() { F<int(int)> f(twice); return f.call(5); }
}
