// [except.spec]/2,13: member noexcept-specifiers are parsed in the
// complete-class context; the function type keeps the written operand
// and its folded value, and a use resolves it.
//@ expect: ast
struct A {
  void f() noexcept(sizeof(int) > 2);
  void g() noexcept(noexcept(f()));
  static void s() noexcept(N == 2);
  void t() throw();
  static constexpr int N = 2;
  int x;
};
void (A::*pg)() = &A::g;
void (*p)() noexcept = &A::s;
