// CWG2351 ([expr.type.conv]/2): `void{}` is a prvalue of type `void`
// that performs no initialization, as `void()` is; usable in a constant
// expression and as a `return` operand of a `void` function. clang 20
// and later.
//@ expect: run
//@ requires: clang>=22
//@ stdout: 1 7
#include <cstdio>
constexpr void g() { return void{}; }
constexpr int h() { void{}; g(); return 7; }
static_assert(h() == 7);
static_assert((void{}, true));
int k = 0;
void m() { ++k; return void{}; }
auto n() -> decltype(void{}) { return void(); }
int main() {
  void{};
  (void)void{};
  m();
  n();
  g();
  std::printf("%d %d\n", k, h());
}
