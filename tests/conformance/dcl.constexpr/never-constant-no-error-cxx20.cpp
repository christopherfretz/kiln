//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
//@ args: -Wno-error=invalid-constexpr
//@ diag: 26:15 warning "constexpr function never produces a constant expression"
//@ diag: 26:29 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 24:5 note "declared here"
//@ diag: 27:15 warning "constexpr function never produces a constant expression"
//@ diag: 27:34 note "read of non-const variable 'glob' is not allowed in a constant expression"
//@ diag: 25:5 note "declared here"
//@ diag: 29:13 warning "constexpr constructor never produces a constant expression"
//@ diag: 29:19 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 24:5 note "declared here"
//@ diag: 30:17 warning "constexpr function never produces a constant expression"
//@ diag: 30:37 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 24:5 note "declared here"
//@ diag: 32:11 warning "constexpr function never produces a constant expression"
//@ diag: 32:40 note "non-constexpr function 'g' cannot be used in a constant expression"
//@ diag: 24:5 note "declared here"
// [dcl.constexpr]/6 is no diagnostic required: from C++23 clang reports a
// constexpr function that can never be constant only under
// -Winvalid-constexpr (off by default); before C++23 it is an error that
// -Wno-error=invalid-constexpr downgrades to a warning. WP-873.
int g();
int glob;
constexpr int f1() { return g(); }
constexpr int f2(int x) { return glob + x; }
struct S {
  constexpr S() { g(); }
  constexpr int mf() const { return g(); }
};
auto l1 = [](int x) constexpr { return g() + x; };
