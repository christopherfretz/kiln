// [temp.names]/3, /5: after `template`, a name whose lookup finds an
// overload set with a function template beside non-template overloads
// names a template. `T::template f<args>()` resolves over the whole set
// at instantiation, as `t.template f<args>()` and the non-dependent
// `C::template f<args>()` do (reflect-cpp's `Literal::contains`).
//@ expect: compile-ok
//@ strict-diags
template <int... N> struct L {
  static bool contains(const char*) { return false; }
  template <int X> static constexpr bool contains() { return X == 3; }
  bool has(int) const { return false; }
  template <int X> constexpr bool has() const { return X == 4; }
};
template <class T> constexpr bool f() { return T::template contains<3>(); }
template <class T> constexpr bool g(T t) { return t.template has<4>(); }
template <class T> struct W {
  template <class U> static constexpr int pick(const U&) {
    if constexpr (T::template contains<3>()) return 1; else return 2;
  }
};
static_assert(f<L<1>>());
static_assert(g(L<1>{}));
static_assert(L<1>::template contains<3>());
static_assert(W<L<1, 2>>::pick(1.0) == 1);
