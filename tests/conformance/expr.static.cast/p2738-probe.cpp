// [expr.static.cast]/14, [expr.const]/5.14 (C++26, P2738R1): a constant
// expression may cast `void*` back to the pointer type of the object it
// points to. WP-185 feature probe.
//@ expect: compile-ok
//@ strict-diags
constexpr int f() {
  int x = 41;
  void* p = &x;
  return *static_cast<int*>(p) + 1;
}
static_assert(f() == 42);
