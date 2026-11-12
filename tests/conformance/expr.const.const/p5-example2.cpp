// [expr.const.const]/5 (P4101R1) Example 2. `h()` passes a pointer to an
// immediate function only within the immediate invocation: OK. The
// example still calls `constexpr auto e = g();` an error ("a pointer to
// an immediate function is not a permitted result"), which /3.1 and the
// struck permitted-result rule of /4 no longer support: kiln rejects it
// as the example says until WP-1328 land 2 decides (P3603). `s.x` of a
// default-initialized `constexpr S s` is erroneous. Compared with GCC
// 16.1.
//@ expect: compile-fail
//@ oracle: gcc
//@ strict-diags
//@ diag: 28:16 error "constexpr variable 'e' must be initialized by a constant expression"
//@ diag: 28:16 note "pointer to a consteval declaration is not a constant expression"
//@ diag: 21:15 note "declared here"
//@ diag: 28:20 error "call to consteval function 'g' is not a constant expression"
//@ diag: 28:20 note "pointer to a consteval declaration is not a constant expression"
//@ diag: 21:15 note "declared here"
//@ diag: 35:15 error "constexpr variable 's' must be initialized by a constant expression"
//@ diag: 35:15 note "subobject 'x' is not initialized"
//@ diag: 31:7 note "subobject declared here"
consteval int f() { return 42; }
consteval auto g() { return f; }
consteval int h(int (*p)() = g()) { return p(); }
constexpr int r = h(); // OK
static_assert(r == 42);

// error: a pointer to an immediate function (see above)
constexpr auto e = g();

struct S {
  int x;
  constexpr S() {}
};
int i() {
  constexpr S s; // error: s.x has erroneous value
  return 0;
}
