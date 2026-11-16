// [expr.const]/22 (before P2686R5): a constant expression may not hold
// the address of an automatic object, the constexpr variable being
// initialized included (`constexpr A a = f();` in a function, where
// `a.self` points to `a`); clang names a constexpr local with
// `note_constexpr_not_static`. From C++26 such a pointer is
// constexpr-referenceable (spec/oven.md). Found running GCC's C++
// testsuite (WP-1468).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:24 error "constexpr variable 'p' must be initialized by a constant expression"
//@ diag: 21:24 note "pointer to 'i' is not a constant expression"
//@ diag: 20:17 note "address of non-static constexpr variable 'i' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
//@ diag: 22:15 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 22:15 note "pointer to 'a' is not a constant expression"
//@ diag: 22:15 note "address of non-static constexpr variable 'a' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
struct A { A *ap = this; };
constexpr A foo() { return {}; }
void f() {
  constexpr int i = 42;
  constexpr const int *p = &i;
  constexpr A a = foo();
}
