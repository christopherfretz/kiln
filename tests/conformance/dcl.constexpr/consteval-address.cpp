// [expr.const]/16-20: naming a consteval function outside an immediate
// function context is an immediate invocation only as a call; in a
// function template specialization (immediate-escalating) the address
// makes the specialization immediate (P2564, clang 21+), so a call of it
// that is not a constant expression is the error.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 31:18 error "cannot take address of consteval function 'sq' outside of an immediate invocation"
//@ diag: 19:15 note "declared here"
//@ diag: 32:12 error "cannot take address of consteval function 'm' outside of an immediate invocation"
//@ diag: 20:26 note "declared here"
//@ diag: 34:20 error "cannot take address of consteval function 'sq' outside of an immediate invocation"
//@ diag: 19:15 note "declared here"
//@ diag: 35:11 error "call to immediate function 'pick<int>' is not a constant expression"
//@ diag: 26:20 note "'pick<int>' is an immediate function because its body evaluates the address of a consteval function 'sq'"
//@ diag: 35:16 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 33:14 note "declared here"
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
