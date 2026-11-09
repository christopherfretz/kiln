// [class.temporary]/6.4 (C++23, P2718R0), [expr.context]/2,
// [expr.static.cast]/6, [expr.comma]/1: a discarded class prvalue in the
// for-range-initializer (the operand of a cast to void, a comma's left
// operand) is materialized, so its temporary lives until the end of the
// loop like every other temporary there, destroyed in reverse order of
// construction. WP-1117.
//@ expect: run
//@ std: c++23
//@ strict-diags
//@ stdout: x1 x2 ~D2 ~D1 | x3 ~D4 ~D3 | end
extern "C" int printf(const char*, ...);
struct D {
  int v;
  ~D() { printf("~D%d ", v); }
};
D mk(int v) { return D{v}; }
int main() {
  int a[] = {1, 2};
  for (int x : ((void)D{1}, mk(2), a)) printf("x%d ", x);
  printf("| ");
  int b[] = {3};
  for (int x : ((void)mk(3), (void)D{4}, b)) printf("x%d ", x);
  printf("| end\n");
}
