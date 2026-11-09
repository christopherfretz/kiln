//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): neither clang 22 nor GCC 16 implements P2686R5 constexpr references to automatic objects (and clang 22 lacks expansion statements)
//@ diag: 14:31 error "constexpr variable 'x' must be initialized by a constant expression"
//@ diag: 14:33 note "read of non-constexpr variable 'a' is not allowed in a constant expression"
//@ diag: 13:7 note "declared here"
// [stmt.expand]/5.2, [expr.const.init]/2 (C++26, P2686R5): binding a local
// range by constexpr reference only needs its address; a constexpr
// for-range-declaration still reads each element's value, which a
// non-constexpr local does not provide ([expr.const]/10), so the element
// is diagnosed, not the range.
void f() {
  int a[2] = {1, 2};
  template for (constexpr int x : a) (void)x;
}
