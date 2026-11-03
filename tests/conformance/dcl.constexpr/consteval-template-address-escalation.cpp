// [expr.const]/18-20 (P2564): a function template specialization whose
// body takes the address of a consteval function outside an immediate
// invocation is an immediate function at clang 21+: a constant call of it
// is fine (also from a consteval function), a non-constant call or taking
// its address is an error naming the reason.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 30:19 error "cannot take address of immediate function 'pick<int>' outside of an immediate invocation"
//@ diag: 20:34 note "declared here"
//@ diag: 21:20 note "'pick<int>' is an immediate function because its body evaluates the address of a consteval function 'sq'"
//@ diag: 32:19 error "cannot take address of immediate function 'keep<long>' outside of an immediate invocation"
//@ diag: 24:34 note "declared here"
//@ diag: 25:13 note "'keep<long>' is an immediate function because its body evaluates the address of a consteval function 'sq'"
//@ diag: 34:11 error "call to immediate function 'keep<int>' is not a constant expression"
//@ diag: 25:13 note "'keep<int>' is an immediate function because its body evaluates the address of a consteval function 'sq'"
//@ diag: 34:16 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 33:14 note "declared here"
consteval int sq(int x) { return x * x; }
template <class T> constexpr int pick(T y) {
  int (*p)(int) = &sq;
  return p(y);
}
template <class T> constexpr int keep(T y) {
  auto p = &sq;
  return y;
}
static_assert(pick(3) == 9);
constexpr int k = keep(4);
int (*fp)(int) = &pick<int>;
consteval int ok(int n) { return pick(n) + keep(n); }
int (*fk)(long) = keep<long>;
void run(int n) {
  int v = keep(n);
}
