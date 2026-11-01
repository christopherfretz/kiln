// [expr.ass]/6 (C++23, P2327R1, extended to every compound assignment by
// CWG2654): compound assignment to a volatile object is not deprecated.
// (`++`/`--` on a volatile still are, per P1152; kiln does not yet warn on
// those, so that half is left to a P1152 test.) WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
volatile int v;
void f() {
  v |= 1;
  v &= 2;
  v ^= 3;
  v <<= 1;
  v += 1;
}
