// [expr.const]/5.16: objects whose lifetime began within the evaluation may be modified.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_16_modify_within_ok`.
struct S { int a; int b[2]; };
constexpr int f() {
  S s = {1, {2, 3}};
  s.a += 10;
  ++s.b[1];
  s.b[0] = s.a;
  return s.a + s.b[0] + s.b[1];
}
static_assert(f() == 26);
