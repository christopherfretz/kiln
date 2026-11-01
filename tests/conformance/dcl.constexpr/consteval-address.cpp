// [expr.const]/16: naming a consteval function outside an immediate
// function context is an immediate invocation only as a call.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:18 error "cannot take address of consteval function 'sq' outside of an immediate invocation"
//@ diag: 11:15 note "declared here"
//@ diag: 24:12 error "cannot take address of consteval function 'm' outside of an immediate invocation"
//@ diag: 12:26 note "declared here"
//@ diag: 26:20 error "cannot take address of consteval function 'sq' outside of an immediate invocation"
//@ diag: 11:15 note "declared here"
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
