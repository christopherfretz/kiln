// [stmt.pre]/1 (C++23, P2360R0): an init-statement may be an
// alias-declaration; the `if` and `switch` forms. The range-based `for`
// form is p2360-for-probe.cpp. WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
constexpr int f() {
  int s = 0;
  if (using U = long; sizeof(U) >= 4) s += 10;
  switch (using V = int; V(2)) { case 2: s += 100; }
  return s;
}
static_assert(f() == 110);
