// [stmt.expand]/5.3 before C++26 (expansion statements as an extension):
// the `constexpr auto&&` that binds a prvalue range refers to a temporary
// with automatic storage duration, which is not a constant expression
// without P2686R5, even when the destructuring is empty. GCC 16.1
// rejects each too.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++23 -Wno-c++26-extensions
//@ requires: clang>=23
//@ oracle: gcc
//@ diag: 21:34 error "constexpr variable '[__u]' must be initialized by a constant expression"
//@ diag: 21:34 note "reference to temporary is not a constant expression"
//@ diag: 21:36 note "temporary created here"
//@ diag: 22:34 error "constexpr variable '[__u]' must be initialized by a constant expression"
//@ diag: 22:34 note "reference to temporary is not a constant expression"
//@ diag: 22:36 note "temporary created here"
struct B { int x = 1, y = 2; };
struct E {};
int f() {
  int s = 0;
  template for (constexpr auto e : B{}) { s += e; }
  template for (constexpr auto e : E{}) { s += 1; }
  return s;
}
