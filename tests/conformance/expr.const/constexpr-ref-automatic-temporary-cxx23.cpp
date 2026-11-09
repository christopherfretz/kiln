//@ expect: compile-fail
//@ strict-diags
//@ std: c++23
//@ diag: 11:24 error "constexpr variable 'r' must be initialized by a constant expression"
//@ diag: 11:24 note "reference to temporary is not a constant expression"
//@ diag: 11:28 note "temporary created here"
// [expr.const]/14 (C++23): before P2686R5 a constexpr reference must refer
// to an object with static storage duration, so a local one bound to a
// temporary it extends is not constant-initialized.
void f() {
  constexpr const int &r = 42;
  (void)r;
}
