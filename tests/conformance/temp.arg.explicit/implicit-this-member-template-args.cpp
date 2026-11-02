// [temp.arg.explicit]/1, [class.mfct.non.static]/2, [temp.inst]/3: a call of a
// member function template through an implicit `this` in a class template,
// with explicit template arguments, keeps them (packs included) when the
// member is instantiated (libc++ test support `throw_error<T>()`).
//@ expect: compile-ok
//@ strict-diags
struct P { struct NC {}; };
template <class C> struct G {
  constexpr int format() const { return pick<P::NC>() + pick<int>(1) + h(1L) + v<int, char>(1, 2); }
  template <class T> constexpr int pick() const { return 1; }
  template <class T> constexpr int pick(T) const { return sizeof(T); }
  template <class T> constexpr int pick(T, int) const { return 100; }
  template <class T> constexpr int h(T) const { return sizeof(T); }
  template <class... T> constexpr int v(T...) const { return sizeof...(T); }
  template <class T> static constexpr int s() { return sizeof(T); }
  static constexpr int t = s<double>();
};
template <class C> struct H {
  template <class T> constexpr int w() const { return 1; }
  template <class U> constexpr int format(U) const { return w<U>() + w<P>(); }
};
static_assert(G<int>{}.format() == 1 + sizeof(int) + sizeof(long) + 2);
static_assert(H<int>{}.format(1) == 2);
static_assert(G<int>::t == sizeof(double));

