// [class.temporary]/8 (CWG3043, C++26 DIS), [expr.context]/2,
// [expr.static.cast]/6, [expr.comma]/1: a discarded class prvalue in an
// enumerating expansion-init-list element (the operand of a cast to void,
// the left operand of a comma) undergoes the temporary materialization
// conversion, so its temporary persists for the lifetime of the
// for-range-declaration too: `~D1` after `x2`. Several in one element are
// destroyed in reverse order of construction ([class.temporary]/11); a
// lambda body's are its own full-expression's; a dependent element is
// materialized when instantiated. In an ordinary declaration nothing is
// extended (class.temporary/discarded-not-extended-run.cpp): `~D11 ~D10`
// before `y12`. GCC 16.1 extends no element temporary; clang 22 has no
// P1306. WP-1106, WP-1117; spec/sema.md §7.6a.
//@ expect: run
//@ strict-diags
//@ requires: clang>=23
//@ oracle: none
//@ stdout: x2 ~D1 | x3 ~D2 x4 ~D3 | x6 ~D5 ~D4 | ~D7 x7 | x8 ~D8 x9 ~D9 | ~D11 ~D10 y12 | end
extern "C" int printf(const char*, ...);
struct D {
  int v;
  ~D() { printf("~D%d ", v); }
};
D mk(int v) { return D{v}; }
template <class T> void dep() {
  template for (int x : {((void)T{8}, 8), (mk(9), 9)}) printf("x%d ", x);
  printf("| ");
}
int main() {
  // A cast to void.
  template for (int x : {((void)D{1}, 2)}) printf("x%d ", x);
  printf("| ");
  // A comma's left operand; a functional cast to void.
  template for (int x : {(mk(2), 3), (void(D{3}), 4)}) printf("x%d ", x);
  printf("| ");
  // Nested: reverse order of construction.
  template for (int x : {((void)D{4}, (void)mk(5), 6)}) printf("x%d ", x);
  printf("| ");
  // A lambda body is not part of the element's full-expression.
  template for (auto x : {[] { return ((void)D{7}, 7); }()}) printf("x%d ", x);
  printf("| ");
  dep<D>();
  int y = ((void)D{10}, (mk(11), 12));
  printf("y%d | end\n", y);
}
