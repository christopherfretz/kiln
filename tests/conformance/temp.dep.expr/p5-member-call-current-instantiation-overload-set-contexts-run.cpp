// [temp.dep.expr]/5, [class.mem.general]/8: calls of an overload set of
// the current instantiation in the other places a member name may be
// used. In a member function's body, a lambda's body (`this` captured,
// a generic lambda too) and a member function of a nested class (whose
// `this` is the nested class) the call resolves at definition, so
// `.m<0>()` names the member template of the result. A requires-clause,
// a trailing return type and a noexcept-specifier of a member are not
// in a complete-class context there (or not for every one): they are
// accepted either way, and a default argument naming a static set
// through `A::` gives its type. WP-1609.
//@ expect: run
//@ skip(clang): clang 22 treats a call through `this` of the current instantiation as type-dependent
struct R { int v; template <int N> int m() { return v + N; } };
template <class T> struct A {
  R ov(int) { return R{1}; }
  R ov(long) { return R{2}; }
  void r() requires (sizeof(A::ov(1)) > 0) {}
  auto tr() -> decltype(this->ov(1)) { return ov(1); }
  void ne() noexcept(noexcept(ov(1))) {}
  static R so(int);
  int d(int x = sizeof(A::so(1))) { return x; }
  struct In {
    R ov(int) { return R{5}; }
    int f() { return ov(1).m<0>(); }
  };
  int f() {
    auto l = [this] { return ov(1).m<0>(); };
    auto g = [&](auto x) { return ov(1L).m<0>() + x; };
    return l() + g(0) + In().f() + tr().v;
  }
};
int main() { A<int> a; a.r(); a.ne(); return a.f() == 1 + 2 + 5 + 1 && a.d() == sizeof(R) ? 0 : 1; }
