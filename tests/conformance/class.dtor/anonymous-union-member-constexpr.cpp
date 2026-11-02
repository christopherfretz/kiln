// [class.dtor]/13, [class.union.anon]/1: a destructor does not invoke the
// (deleted) destructor of an anonymous union member, whose members are
// variant members; in constant evaluation it is not "undefined". libc++
// <optional>:291 (__optional_destruct_base<T, false>; optional
// explicit_optional_U, WP-276). Verified against clang 19.
//@ expect: compile-ok
//@ strict-diags
struct X {
  int v;
  constexpr X(int v) : v(v) {}
  constexpr ~X() {}
};
struct O {
  union { char n; X x; };
  bool e;
  constexpr O() : n(), e(false) {}
  constexpr O(int v) : x(v), e(true) {}
  constexpr ~O() { if (e) x.~X(); }
};
constexpr bool t1() { O a; O b(3); return !a.e && b.x.v == 3; }
static_assert(t1());
template <class T> struct Opt {
  union { char n; T t; };
  bool e = false;
  constexpr Opt() : n() {}
  constexpr ~Opt() { if (e) t.~T(); }
};
constexpr bool t2() { Opt<X> a; Opt<int> b; return !a.e && !b.e; }
static_assert(t2());
