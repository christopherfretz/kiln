// [expr.const]/18 (P2564): a constexpr function template specialization
// that calls a consteval function with a non-constant becomes immediate.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:11 error "call to immediate function 'twice<int>' is not a constant expression"
//@ diag: 17:56 note "'twice<int>' is an immediate function because its body contains a call to a consteval function 'sq' and that call is not a constant expression"
//@ diag: 24:17 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 22:13 note "declared here"
//@ diag: 25:11 error "call to immediate function 'outer<int>' is not a constant expression"
//@ diag: 18:52 note "'outer<int>' is an immediate function because its body contains a call to an immediate function 'twice<int>' and that call is not a constant expression"
//@ diag: 25:17 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 22:13 note "declared here"
//@ diag: 28:11 error "call to immediate function 'run(int)::(anonymous class)::operator()' is not a constant expression"
//@ diag: 28:13 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 22:13 note "declared here"
consteval int sq(int x) { return x * x; }
template <class T> constexpr T twice(T y) { return 2 * sq(y); }
template <class T> constexpr T outer(T y) { return twice(y) + 1; }
constexpr int plain(int y) { return sq(2) + y; }
static_assert(twice(3) == 18);
static_assert(outer(3) == 19);
int run(int n) {
  int a = twice(4);
  int b = twice(n);
  int c = outer(n);
  int d = plain(n);
  auto l = [](int y) { return sq(y); };
  int e = l(n);
  return a + b + c + d + e;
}
