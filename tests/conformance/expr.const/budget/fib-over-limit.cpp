// [expr.const]/5.1, [implimits]: one step below the minimum fib.cpp needs:
// the evaluation stops at the step limit on both kiln and clang 19.
//@ args: -fconstexpr-steps=6385
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:15 error "static assertion expression is not an integral constant expression"
//@ diag: 18:3 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 18:35 note "in call to 'fib(0)'"
//@ diag: 18:35 note "in call to 'fib(2)'"
//@ diag: 18:35 note "in call to 'fib(4)'"
//@ diag: 18:35 note "in call to 'fib(6)'"
//@ diag: 18:35 note "in call to 'fib(8)'"
//@ diag: 18:35 note "in call to 'fib(10)'"
//@ diag: 18:35 note "in call to 'fib(12)'"
//@ diag: 18:35 note "in call to 'fib(14)'"
//@ diag: 21:15 note "in call to 'fib(16)'"
constexpr unsigned long long fib(unsigned n) {
  return n < 2 ? n : fib(n - 1) + fib(n - 2);
}
static_assert(fib(10) == 55);
static_assert(fib(16) == 987);
