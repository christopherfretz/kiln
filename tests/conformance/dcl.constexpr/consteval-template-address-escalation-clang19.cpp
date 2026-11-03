// [expr.const]/18-20: clang 19 does not escalate a function template
// specialization for the address of a consteval function its body takes
// (it shares the definition's reference), and accepts every use below.
//@ expect: compile-ok
//@ requires: clang<22
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
