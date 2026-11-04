// [expr.const]/16 (P2564R3): a call to a void consteval function outside
// an immediate-escalating context is an immediate invocation; it is a
// constant expression of type void (it has no value), whether it is an
// expression statement, the operand of a cast to void or of a comma, or
// a nested call in a constexpr function. Nothing is emitted for it at
// run time (clang's ConstantExpr of type void generates no code; WP-537).
//@ expect: run
//@ strict-diags
//@ stdout: 1
extern "C" int printf(const char*, ...);
consteval void h() {}
consteval void h2(int) {}
constexpr int g() { h(); h2(3); (void)h(); return 1; }
static_assert(g() == 1);
int main() {
  h();
  (void)h2(1);
  h(), h();
  printf("%d\n", g());
  return 0;
}
