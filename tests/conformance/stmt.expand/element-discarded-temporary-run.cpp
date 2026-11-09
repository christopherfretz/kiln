// [class.temporary]/8 (CWG3043, C++26 DIS), [expr.context]/2,
// [expr.static.cast]/6: a discarded class prvalue in an enumerating
// expansion-init-list element (cast to void, the left operand of a comma)
// is materialized, so its temporary persists for the lifetime of the
// for-range-declaration too: `~D1` after `x2`. kiln destroys it at the end
// of the declaration's full-expression, as GCC 16.1 does (it extends no
// element temporary); clang 22 has no P1306.
// WP-1106; spec/sema.md §7.6a.
//@ expect: run
//@ strict-diags
//@ requires: clang>=23
//@ oracle: none
//@ xfail(kiln): WP-1106 follow-up, a discarded class prvalue is bound (CXXBindTemporaryExpr) but never materialized, so it is not extended
//@ stdout: x2 ~D1 | end
extern "C" int printf(const char*, ...);
struct D {
  int v;
  ~D() { printf("~D%d ", v); }
};
int main() {
  template for (int x : {((void)D{1}, 2)}) printf("x%d ", x);
  printf("| end\n");
}
