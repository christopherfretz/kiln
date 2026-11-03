// [expr.const]/16: naming a consteval function outside an immediate
// function context is an immediate invocation only as a call.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:18 error "cannot take address of consteval function 'sq' outside of an immediate invocation"
//@ diag: 13:15 note "declared here"
//@ diag: 26:12 error "cannot take address of consteval function 'm' outside of an immediate invocation"
//@ diag: 14:26 note "declared here"
//@ diag: 28:20 error "cannot take address of consteval function 'sq' outside of an immediate invocation"
//@ diag: 13:15 note "declared here"
//@ requires: clang<22
// WP-355 residual: clang 22 escalates a function template taking a consteval function's address (P2564) and reports the escalated call with its notes; kiln does not escalate it.
consteval int sq(int x) { return x * x; }
struct S { consteval int m() const { return 1; } };
consteval int via_ptr(int x) {
  int (*p)(int) = sq;
  return p(x);
}
template <class T> constexpr int pick(T y) {
  int (*p)(int) = &sq;
  return p(y);
}
static_assert(via_ptr(3) == 9);
static_assert(pick(2) == 4);
int (*gp)(int) = sq;
auto pm = &S::m;
void run(int n) {
  int (*p)(int) = &sq;
  int v = pick(n);
}
