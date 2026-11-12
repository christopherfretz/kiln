// [expr.const.const]/5 (P4101R1) Example 2, the lines GCC 16.1 agrees on.
// `h()` passes a pointer to an immediate function only within the
// immediate invocation: OK. `s.x` of a default-initialized `constexpr S
// s` is erroneous. The example's `constexpr auto e = g();` is in
// `p5-example2-immediate-result.cpp` (kiln accepts it, GCC 16.1 does
// not). Compared with GCC 16.1.
//@ expect: compile-fail
//@ oracle: gcc
//@ strict-diags
//@ diag: 27:15 error "constexpr variable 's' must be initialized by a constant expression"
//@ diag: 27:15 note "subobject 'x' is not initialized"
//@ diag: 23:7 note "subobject declared here"
consteval int f() { return 42; }
consteval auto g() { return f; }
consteval int h(int (*p)() = g()) { return p(); }
constexpr int r = h(); // OK
static_assert(r == 42);

// (`constexpr auto e = g();`: see the file comment.)
static_assert(g()() == 42);

struct S {
  int x;
  constexpr S() {}
};
int i() {
  constexpr S s; // error: s.x has erroneous value
  return 0;
}
