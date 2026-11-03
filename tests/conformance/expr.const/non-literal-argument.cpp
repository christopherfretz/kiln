// [expr.const]/10, [expr.call]/7, [expr.call]/12, P2448R2: from C++23 clang 22
// no longer checks that a by-value argument has literal type. A parameter
// object of non-literal class type is rejected through its non-constexpr
// destructor, run at the end of the call and located at the parameter; a
// class with a non-trivial copy constructor or destructor passed through the
// ellipsis is clang's `(__builtin_trap(), arg)` (conditionally-supported), so
// the evaluation fails at the argument with no note.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 21:24 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 22:15 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 19:19 note "non-constexpr function '~NL' cannot be used in a constant expression"
//@ diag: 16:59 note "declared here"
//@ diag: 23:27 error "constexpr variable 'c' must be initialized by a constant expression"
struct NL { constexpr NL() {} constexpr NL(const NL &) {} ~NL() {} };
struct L { int x = 0; };
constexpr int v(int, ...) { return 1; }
constexpr int h(NL) { return 1; }
constexpr int lit = v(1, L());
constexpr int a = v(1, NL());
constexpr int b = h(NL());
constexpr int c = v(1, 2, (NL()));
