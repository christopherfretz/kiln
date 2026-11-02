// [stmt.pre]/1, [stmt.ranged]/1 (C++23, P2360R0): the init-statement of a
// range-based `for` may be an alias-declaration. WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
constexpr int f() {
  int s = 0, arr[] = {1, 2, 3};
  for (using T = int; T i : arr) s += i;
  return s;
}
static_assert(f() == 6);
