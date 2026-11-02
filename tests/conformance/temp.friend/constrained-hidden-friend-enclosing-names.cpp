// [temp.friend]/9, [class.friend]/7, [temp.inst]/17, [temp.constr.atomic]/3:
// the requires-clause of a hidden friend of a member class (template) of a
// class template names the enclosing template's parameters, a member
// typedef, a static data member and a class-scope function that hides a
// namespace one; it is checked with the arguments of the specialization
// that declared the friend. Matches clang 19 (WP-278 audit).
//@ expect: compile-ok
//@ strict-diags
template <class T> concept Small = sizeof(T) <= 4;
template <class T> struct Outer {
  using value_type = T;
  static constexpr bool ok = Small<T>;
  template <bool B> struct Inner {
    int v = 0;
    friend constexpr bool f(Inner) requires Small<T> && B { return true; }
    friend constexpr bool g(Inner) requires Small<value_type> { return true; }
    friend constexpr bool h(Inner) requires ok { return true; }
    friend constexpr bool m(Inner) requires (sizeof(Inner) == sizeof(int)) { return true; }
  };
};
template <class X> concept CallF = requires(X x) { f(x); };
template <class X> concept CallG = requires(X x) { g(x); };
template <class X> concept CallH = requires(X x) { h(x); };
template <class X> concept CallM = requires(X x) { m(x); };
static_assert(CallF<Outer<int>::Inner<true>>);
static_assert(!CallF<Outer<int>::Inner<false>>);
static_assert(!CallF<Outer<double>::Inner<true>>);
static_assert(CallG<Outer<int>::Inner<true>> && !CallG<Outer<double>::Inner<true>>);
static_assert(CallH<Outer<int>::Inner<true>> && !CallH<Outer<double>::Inner<true>>);
static_assert(CallM<Outer<int>::Inner<true>>);

namespace n {
template <class T> constexpr bool test(T) { return false; }
template <class T> struct Outer2 {
  template <class U> static constexpr bool test(U) { return true; }
  struct Inner {
    friend constexpr bool f(Inner) requires (test(T{})) { return true; }
  };
};
}  // namespace n
static_assert(CallF<n::Outer2<int>::Inner>);
