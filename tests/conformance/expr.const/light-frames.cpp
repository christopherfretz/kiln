// [expr.const]/10, [implimits]: calls of functions that make no frame
// object (the Oven's light frames, spec/oven.md "Light frames") keep
// every diagnostic of a call: the "in call to" note of each call, a
// pointer to a caller's object that dies after the call returns, and the
// depth limit with its backtrace.
//@ args: -fconstexpr-depth=20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 36:15 error "static assertion expression is not an integral constant expression"
//@ diag: 30:44 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 31:44 note "in call to 'deref(&a)'"
//@ diag: 34:10 note "in call to 'twice(&a)'"
//@ diag: 36:15 note "in call to 'uninit()'"
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:10 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 46:15 note "in call to 'local_out()'"
//@ diag: 48:15 error "static assertion expression is not an integral constant expression"
//@ diag: 47:49 note "constexpr evaluation exceeded maximum depth of 20 calls"
//@ diag: 47:49 note "in call to 'down(1)'"
//@ diag: 47:49 note "in call to 'down(2)'"
//@ diag: 47:49 note "in call to 'down(3)'"
//@ diag: 47:49 note "in call to 'down(4)'"
//@ diag: 47:49 note "in call to 'down(5)'"
//@ diag: 47:49 note "(skipping 10 calls in backtrace; use -fconstexpr-backtrace-limit=0 to see all)"
//@ diag: 47:49 note "in call to 'down(16)'"
//@ diag: 47:49 note "in call to 'down(17)'"
//@ diag: 47:49 note "in call to 'down(18)'"
//@ diag: 47:49 note "in call to 'down(19)'"
//@ diag: 48:15 note "in call to 'down(20)'"
constexpr int deref(const int *p) { return *p; }
constexpr int twice(const int *p) { return deref(p) + deref(p); }
constexpr int uninit() {
  int a;
  return twice(&a);
}
static_assert(uninit() == 0);
constexpr const int *addr(const int &r) { return &r; }
constexpr int local_out() {
  const int *p = nullptr;
  {
    int x = 1;
    p = addr(x);
  }
  return *p;
}
static_assert(local_out() == 1);
constexpr int down(int n) { return n == 0 ? 0 : down(n - 1); }
static_assert(down(20) == 0);
static_assert(down(19) == 0);
