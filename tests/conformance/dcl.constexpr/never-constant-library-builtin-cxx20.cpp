// [dcl.constexpr]/6, [expr.const]/10: a library builtin declared with C
// linkage. One clang constant-evaluates (`strlen`, a `Constexpr` library
// builtin) is noted as a non-constexpr function at the call whatever its
// arguments, with no "declared here"; any other (`fabs`, `abs`) is an
// ordinary non-constexpr function: its arguments are evaluated first, so
// only a call with known arguments fails, with a "declared here" note.
// kiln's Oven folds both under libstdc++ (as GCC does); the never-constant
// check follows clang 19 (WP-945). spec/sema.md "Never constant".
//@ std: c++20
//@ strict-diags
//@ expect: compile-fail
//@ diag: 30:25 error "constexpr function never produces a constant expression"
//@ diag: 30:52 note "non-constexpr function 'strlen' cannot be used in a constant expression"
//@ diag: 31:25 error "constexpr function never produces a constant expression"
//@ diag: 31:44 note "non-constexpr function 'strlen' cannot be used in a constant expression"
//@ diag: 32:25 error "constexpr function never produces a constant expression"
//@ diag: 32:39 note "non-constexpr function 'strlen' cannot be used in a constant expression"
//@ diag: 33:18 error "constexpr function never produces a constant expression"
//@ diag: 33:40 note "non-constexpr function 'fabs' cannot be used in a constant expression"
//@ diag: 28:19 note "declared here"
//@ diag: 34:15 error "constexpr function never produces a constant expression"
//@ diag: 34:38 note "non-constexpr function 'abs' cannot be used in a constant expression"
//@ diag: 29:16 note "declared here"
//@ diag: 35:18 error "constexpr function never produces a constant expression"
//@ diag: 35:32 note "non-constexpr function 'fabs' cannot be used in a constant expression"
//@ diag: 28:19 note "declared here"
extern "C" unsigned long strlen(const char *);
extern "C" double fabs(double);
extern "C" int abs(int);
constexpr unsigned long l1(const char *s) { return strlen(s); }
constexpr unsigned long l2(int x) { return strlen("ab") + x; }
constexpr unsigned long l3() { return strlen("ab"); }
constexpr double l4(double x) { return fabs(2.0) + x; }
constexpr int l5(int x) { return x + abs(-2); }
constexpr double l6() { return fabs(-2.0); }
