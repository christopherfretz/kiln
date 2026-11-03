// A friend declared in a class template whose trailing return type
// accesses members of the current instantiation through a parameter
// (`decltype(l.base() - r.base())`, libstdc++ 16 <bits/stl_iterator.h>
// `__normal_iterator` `operator-`; WP-525): the access was resolved in the
// pattern, so when the class is instantiated -- and still incomplete --
// it is formed again with the member found, without requiring the class
// to be complete ([temp.inst]/3, [temp.dep.type]/1; clang
// `TreeTransform::RebuildMemberExpr`). Verified against clang 19.
//@ expect: compile-ok
//@ strict-diags
template <class I, class C> struct It {
  I p;
  constexpr I base() const { return p; }
  template <class J>
  friend constexpr auto operator-(const It& l, const It<J, C>& r)
      -> decltype(l.base() - r.base()) {
    return l.base() - r.base();
  }
};
struct S {};
constexpr const char* text = "abc";
static_assert(It<const char*, S>{text + 2} - It<const char*, S>{text} == 2);
template <class T> struct Y {
  constexpr int base() const { return 4; }
  friend constexpr auto f(const Y& l) -> decltype(l.base()) { return l.base(); }
};
static_assert(f(Y<int>{}) == 4);
template <class T> struct Z {
  constexpr int base() const { return 5; }
  template <class U>
  friend constexpr auto h(const Z& l, U) -> decltype(l.base()) { return l.base(); }
};
static_assert(h(Z<int>{}, 1) == 5);
