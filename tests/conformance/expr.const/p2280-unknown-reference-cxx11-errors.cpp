// [expr.const]/10 (P2280R4, a defect report): in C++11 too, the object an
// unknown reference binds to has an unknown value, so reading it and
// comparing its address with null are not constant (the notes clang 22
// gives from C++23).
//@ expect: compile-fail
//@ strict-diags
//@ std: c++11
//@ args: -Wno-tautological-undefined-compare
//@ xfail(clang): clang 22 applies P2280 from C++23 only (its notes differ before)
//@ diag: 18:17 error "static assertion expression is not an integral constant expression"
//@ diag: 16:43 note "read of variable 'x' whose value is not known"
//@ diag: 18:17 note "in call to 'read(x)'"
//@ diag: 17:19 note "declared here"
//@ diag: 19:17 error "static assertion expression is not an integral constant expression"
//@ diag: 19:20 note "comparison between pointers to unrelated objects '&x' and 'nullptr' has unspecified value"
constexpr int read(const int &r) { return r; }
void f(const int &x) {
  static_assert(read(x) == 0, "");
  static_assert(&x != nullptr, "");
}
