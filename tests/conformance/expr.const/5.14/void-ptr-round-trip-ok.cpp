// [expr.const]/5.14: a `void*` conversion back to the object's type is allowed (C++26).
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_14_void_ptr_round_trip_ok`.
constexpr int f() {
  int x = 7;
  void *v = &x;
  return *static_cast<int *>(v);
}
static_assert(f() == 7);
