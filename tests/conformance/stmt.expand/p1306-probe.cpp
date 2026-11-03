// [stmt.expand] (C++26, P1306R5): expansion statements `template for` over
// an expansion-init-list. WP-185 feature probe; passing since WP-447 (the
// body is a compound-statement, as P1306R5's grammar requires).
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=23
constexpr int sum() {
  int s = 0;
  template for (constexpr int i : {1, 2, 3}) { s += i; }
  return s;
}
static_assert(sum() == 6);
