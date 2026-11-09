// [expr.const]/10.8, [class.mfct.non.static]/2: a member call on qa[3] (one
// past the end) is undefined. Found by the clang negative-test run (WP-912).
//@ std: c++23
//@ expect: compile-fail
//@ diag: 11:15 error "constexpr variable 'k' must be initialized by a constant expression"
//@ diag: 11:25 note "member call on dereferenced one-past-the-end pointer is not allowed in a constant expression"
struct Q {
  constexpr int f() const { return 0; }
};
Q qa[3];
constexpr int k = qa[3].f();
