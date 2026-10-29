// [stmt.if]/2: the condition of a constexpr if statement shall be a
// contextually converted constant expression of type bool
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:17 error "constexpr if condition is not a constant expression"
//@ diag: 13:17 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 11:5 note "declared here"
//@ diag: 19:17 error "constexpr if condition is not a constant expression"
//@ diag: 19:17 note "function parameter 'v' with unknown value cannot be used in a constant expression"
//@ diag: 18:11 note "declared here"
int n = 3;
int f(int v) {
  if constexpr (n > 2) {
    return v;
  }
  return 0;
}
int g(int v) {
  if constexpr (v) {
    return 1;
  }
  return 0;
}
