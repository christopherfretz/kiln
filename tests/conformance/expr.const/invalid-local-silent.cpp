// [expr.const]/10, [dcl.init.general]: a local variable whose
// initialization sema already rejected makes its constexpr function's
// evaluation fail with no further note, so each constant expression that
// calls it reports only its own error (clang marks the variable invalid).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:23 error "no matching constructor for initialization of 'A'"
//@ diag: 14:22 note "candidate constructor not viable: requires 1 argument, but 0 were provided"
//@ diag: 14:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 14:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 16:15 error "static assertion expression is not an integral constant expression"
//@ diag: 17:15 error "constexpr variable 'k' must be initialized by a constant expression"
struct A { constexpr A(int) {} };
constexpr int f() { A x; return 0; }
static_assert(f() == 0);
constexpr int k = f();
