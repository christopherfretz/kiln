// Mangling differential: constrained templates (spec/abi.md §8.16,
// WP-204). Type-constraints on template parameters (`Tk`), template
// template parameter heads (`Tt ... E`), constrained placeholders
// (`Dk`), template-head and trailing requires-clauses (`Q`), and the
// constraint expressions inside them: concept-ids, folds,
// requires-expressions with `fp`/`fL` parameter references, member
// access, construction, `new` (also braced), and member-like constrained friends.

namespace std {
template <class T, class U> concept same_as = __is_same(T, U);
template <class T> concept integral = __is_integral(T);
} // namespace std

template <class T> concept C = sizeof(T) >= 1;
template <class T, class U> concept D = sizeof(T) == sizeof(U);
namespace N {
template <class T> concept E = true;
namespace M {
template <class T, class U> concept Conv = __is_convertible(T, U);
template <class T> concept K = sizeof(T) > 1;
} // namespace M
} // namespace N

// ---- type-constraints and template-head requires-clauses --------------
template <C T> void f1(T) {}
template <D<int> T> void f2(T) {}
template <class T> requires C<T> void f3(T) {}
template <class T> void f4(T) requires C<T> {}
void f5(C auto) {}
template <C auto V> void f6() {}
template <C... Ts> void f7(Ts...) {}
template <template <class> class TT> void f8() {}
template <template <C> class TT> void f9() {}
template <class T> struct S {};
template <C T> struct CS {};
template <N::E T> void f10(T) {}
template <class T> C auto f11(T t) { return t; }
template <class T> requires C<T> && D<T, int> void f12(T) {}
template <typename T> void f13(T) requires (sizeof(T) > 2) {}
template <class T, D<T> U> void f14(T, U) {}
template <class T> void f15(T) requires C<T> || C<T *> {}
template <C T> void f16(CS<T>) {}
template <class T> void f17(T) requires (!D<T, char>) {}
template <class T> void f18(T) requires true {}
template <class... Ts> void f19(Ts...) requires (C<Ts> && ...) {}
template <D<int>... Ts> void f20(Ts...) {}
template <class T> void f21(T) requires C<typename T::type> {}
struct Q { using type = int; };
template <std::same_as<int> T> void f22(T) {}
template <std::integral T> void f23(T) {}
template <N::M::Conv<long> T> void f24(T) {}
template <N::M::K T> void f25(T) {}
template <std::integral auto V> void f26() {}
template <C T, C U> void f27(T, U) {}
template <class T> decltype(auto) f28(T t) { return t; }

// ---- members of class templates: constraints keep their depth ----------
template <class T> struct Mt {
  template <class U> void g(U) requires C<U> {}
  void h() requires C<T> {}
  template <D<T> U> void k(U) {}
  template <class U> requires D<T, U> void j(U) {}
  template <class U> requires std::same_as<U, T> void m(U) {}
  template <class U> struct I {
    template <D<U> W> void k(W) {}
    template <class W> void j(W) requires D<T, W> && D<U, W> {}
  };
};
template <class T> requires C<T> struct R { void h() {} };

// ---- template template parameters -------------------------------------
template <class U, U V> struct X {};
template <class U, int V> struct Y {};
template <class T> struct Z {};
template <class... Ts> struct P {};
template <template <class U, U V> class TT> void h1() {}
template <template <class U, int V> class TT> void h2() {}
template <template <class> class TT> requires C<TT<int>> void h3() {}
template <template <class U> requires C<U> class TT> void h4() {}
template <template <class...> class TT> void h5() {}
template <template <C...> class TT> void h6() {}

// ---- requires-expressions and parameter references ---------------------
template <class T> void g1(T x) requires requires { x + 1; } {}
template <class T> void g2(T x) requires C<decltype(x)> {}
template <class T> void g3(T, T y) requires requires (T a, T b) {
  a + b; y; requires C<T>; typename T; { a } -> C; { b } noexcept;
} {}
template <class T> requires requires (T a) { a; } void g4(T) {}
template <class T> void g5(T) requires requires { requires requires (T z) { z; }; } {}
template <class T> constexpr bool B = true;
template <class T> void g6(T) requires B<T> {}
template <class T> void g7(T) requires (C<T>) {}
template <class T> void g8(T) requires requires (T t) { t + 1; } {}
template <class T> void g9(T *t, T &r) requires requires { *t; r; } {}
template <class T> void g10(T t) requires requires { { t + 1 } noexcept -> std::same_as<T>; } {}
template <class T> void g11(T) requires requires (T u) { u.~T(); } {}

// ---- folds ---------------------------------------------------------------
template <class... Ts> requires (C<Ts> && ... && true) void k1(Ts...) {}
template <class... Ts> requires (true || ... || C<Ts>) void k2(Ts...) {}
template <class... Ts> requires ((sizeof(Ts) + ...) > 2) void k3(Ts...) {}

// ---- expressions in requirements ----------------------------------------
struct V {
  int size() const { return 0; }
  int x;
  using value_type = int;
  int operator[](int) const { return 0; }
};
template <class T> T &&declval();
template <class T> void e1(T t) requires requires { t.size(); } {}
template <class T> void e2(T t) requires requires { t.x; } {}
template <class T> void e3(T) requires requires { typename T::value_type; } {}
template <class T> void e4(T t) requires requires { t[0]; } {}
template <class T> void e5(T) requires requires { declval<T>().size(); } {}
template <class T> void e6(T) requires requires (T &u) { ++u.x; u.x = 1; } {}
template <class T> void e7(T) requires (sizeof(T) == 4 && alignof(T) == 4) {}
template <class T> void e8(T) requires requires { T{}; T(); new T; } {}
template <class T> void e9(T t) requires requires { static_cast<int>(t.x); } {}
template <class T> void e10(T *p) requires requires { p->x; p->size(); } {}
struct W2 { W2(int, int); W2(); };
template <class T> void e11(T) requires requires { new T(1, 2); ::new T(); new T[2]; } {}
// Braced new-initializers: `il <braced-expression>* E` (WP-231).
struct W3 { int a, b; };
struct W4 { W3 w; int c; };
template <class T> void e12(T) requires requires { new T{}; new T{1, 2}; ::new T{1}; } {}
template <class T> void e13(T t) requires requires { new T[2]{}; new T[2]{t, t}; new T{t}; } {}
template <class T, class... A> void e14(T, A... a) requires requires { new T{{1, 2}, 3}; new T{a...}; } {}

// ---- constrained special members and member-like friends -----------------
template <class T> struct Xs {
  Xs() requires C<T> {}
  Xs(int) requires C<T> {}
  ~Xs() requires C<T> {}
  ~Xs() {}
  operator int() const requires C<T> { return 0; }
  friend void fr(Xs) requires C<T> {}
  friend bool operator==(Xs, Xs) requires C<T> { return true; }
  friend void gr(Xs) {}
  static void s() requires C<T> {}
};
template <class T> struct Ps;
template <C T> struct Ps<T> {
  void m() requires N::M::K<T> {}
  void n() {}
};

// Member-like friend templates (clang 19's form; kiln's mangler runs at
// -fclang-compat=19 here, WP-1146): a nested name for a specialization of
// a friend with a trailing requires-clause, `F` for one constrained on the
// enclosing class's parameter.
struct Fs {
  template <class U> friend void mf1(U, Fs) requires C<U> {}
  template <class U> friend void mf2(U, Fs) requires C<U>;
};
template <class U> void mf2(U, Fs) requires C<U> {}
template <class U> void mf3(U) requires C<U>;
struct Fs3 {
  template <class U> friend void mf3(U) requires C<U> {}
};
template <class T> struct Xt {
  template <class U> friend void mf4(U, Xt) requires C<U> {}
  template <class U> friend void mf5(U, Xt) requires C<T> {}
  template <class U> requires C<T> friend void mf6(U, Xt) {}
  template <class U> friend void mf7(U, Xt) requires C<U> && C<T> {}
};

void use() {
  mf1(1, Fs{}); mf2(1, Fs{}); mf3(1); Xt<int> xt; mf4(1, xt); mf5(1, xt); mf6(1, xt); mf7(1, xt);
  f1(1); f2(1); f3(1); f4(1); f5(1); f6<1>(); f7(1, 2.0); f7(); f8<S>(); f9<CS>(); f9<S>();
  f10(1); f11(1); f12(1); f13(1); f14(1, 2); f15(1); f16(CS<int>{}); f17(1); f18(1);
  f19(1, 2); f20(1, 2); f21(Q{}); f22(1); f23(2); f24(3); f25(4L); f26<3>(); f27(1, 'c');
  f28(1);
  Mt<int>{}.g(1); Mt<int>{}.h(); Mt<int>{}.k(1); Mt<int>{}.j(1); Mt<int>{}.m(1);
  Mt<int>::I<int>{}.k(1); Mt<int>::I<int>{}.j(1); R<int>{}.h();
  // `h2<X>` and `h6<CS>` are ill-formed under clang 22's matching
  // (conformance/temp.arg.template/deduction-and-constraints-clang22.cpp).
  h1<X>(); h2<Y>(); h3<Z>(); h4<Z>(); h5<P>(); h5<Z>(); h6<P>();
  g1(1); g2(1); g3(1, 2); g4(1); g5(1); g6(1); g7(1); g8(1); int i; g9(&i, i); g10(1); g11(1);
  k1(1, 2.0); k2(1); k3(1, 2);
  V v; e1(v); e2(v); e3(v); e4(v); e5(v); e6(v); e7(v); e8(v); e9(v); e10(&v); e11(W2());
  e12(W3{}); e13(W3{}); e14(W4{}, W3{}, 3);
  Xs<int> x; Xs<int> y(1); (void)int(x); fr(x); (void)(x == x); gr(x); Xs<int>::s();
  Ps<long> p; p.m(); p.n();
  auto l = [](C auto a) { return a; }; l(1);
  auto l2 = [](auto a) requires C<decltype(a)> { return a; }; l2(1);
}
