// [temp.inst], [dcl.pre]/10, spec/modules.md §11.11 Part 2 (WP-1046):
// failing static_asserts and an ill-formed instantiation that read
// constants a kind-2 module file carries from the module's own
// instantiations. The diagnostics are those of kind 1 and of the
// in-process build (`--bmi-kind 1|2`).
//@ expect: compile-fail
//@ std: c++20
//@ module: ct=Inputs/ct.cppm
//@ two-step
//@ strict-diags
//@ diag: 20:15 error "static assertion failed due to requirement 'Fib<20>::value == 6764'"
//@ diag: 20:30 note "expression evaluates to '6765 == 6764'"
//@ diag: 21:15 error "static assertion failed due to requirement 'twice<17>() < 0'"
//@ diag: 21:27 note "expression evaluates to '3194 < 0'"
//@ diag: 22:15 error "static assertion failed due to requirement 'Arr<int, 19>::size == use'"
//@ diag: 22:34 note "expression evaluates to '3 == 12546'"
//@ diag: 24:15 error "static assertion failed due to requirement 'Tag<2584>::value != 2584'"
//@ diag: 24:34 note "expression evaluates to '2584 != 2584'"
import ct;
static_assert(Fib<20>::value == 6764);
static_assert(twice<17>() < 0);
static_assert(Arr<int, 19>::size == use);
Tag<Fib<18>::value> t;
static_assert(decltype(t)::value != 2584);
int ok = Fib<20>::big ? 1 : 0;
