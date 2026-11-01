// [expr.type.conv]/1 (C++23, P0849R8): `auto(x)` and `auto{x}` make a
// prvalue copy of `x` with its decayed type. WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ xfail(kiln): auto(x) / auto{x} not implemented
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
struct S { int v; };
constexpr int f() {
  int a[2] = {1, 2};
  const S s{7};
  static_assert(same<decltype(auto(a)), int*>);
  static_assert(same<decltype(auto{s}), S>);
  S c = auto(s);
  return c.v + *auto(a);
}
static_assert(f() == 8);
