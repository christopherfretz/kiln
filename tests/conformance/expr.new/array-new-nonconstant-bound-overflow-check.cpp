// [expr.new]/8, [expr.pre]/4: -Winteger-overflow's evaluation of a
// full-expression (clang `EvaluateForOverflow`) stops at an array
// new-expression whose bound is not a constant (clang
// `VisitCXXNewExpr` returns at the bound): the placement arguments and
// the bound are searched for overflows, the initializer is not, so an
// overflow in the elements' initializers warns only with a constant
// bound. The element constructor of `new S[n]` (typed `S[]`) was
// evaluated as an expression of its own and crashed the constant
// evaluator when the new-expression was an operand (`p = new S[n]`).
// Reduced from LLVM 19 (`DeclObjC.cpp`, `Stmt.cpp` and three more).
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
//@ diag: 53:30 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 54:30 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 55:36 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
struct S { unsigned x = 0; };
struct C { constexpr C() {} constexpr C(int) {} int v = 0; };
S *p;
C *pc;
int *pi;
void plain(unsigned long n) {
  p = new S[n];
  p = new S[n]();
  p = (new S[n]);
  pc = new C[n];
  p = n ? new S[n] : nullptr;
  p = nullptr, p = new S[n];
  int k = 0;
  k += (p = new S[n + 1]) ? 1 : 0;
  auto l = [&] { p = new S[n]; };
  l();
}
template <class T> void tmpl(unsigned long n) {
  T *q;
  q = new T[n];
  p = new S[n];
}
template void tmpl<S>(unsigned long);
constexpr int cx(int n) {
  S *q;
  q = new S[n];
  int r = q[n - 1].x;
  delete[] q;
  return r;
}
static_assert(cx(3) == 0);
void overflow(unsigned long n) {
  pi = new int[n]{2147483647 + 1};
  pc = new C[n](2147483647 + 1);
  pc = new C[n]{C(2147483647 + 1)};
  pc = new C[n]{2147483647 + 1};
  pc = new C[2]{C(2147483647 + 1)};
  pi = new int[2]{2147483647 + 1};
  pi = new int[n + 0 * (2147483647 + 1)];
}
