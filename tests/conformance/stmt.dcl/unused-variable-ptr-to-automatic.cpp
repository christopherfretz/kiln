// [stmt.dcl], [dcl.attr.unused], [expr.const.init] (WP-467): a local of
// class type built by a non-trivial constexpr constructor is diagnosed
// unused only when its initializer is a constant (clang
// ShouldDiagnoseUnusedDecl). One that points to an automatic object is
// constant at C++26 by P2686R5, which clang does not implement, so it
// stays undiagnosed like clang (libc++ span.cons/stdarray.pass.cpp).
//@ args: -Wunused-variable
//@ strict-diags
//@ diag: 22:5 warning "unused variable 'g'"
struct P {
  constexpr P(int *q) : p(q) {}
  int *p;
};
int gl;
void f() {
  int arr[3] = {1, 2, 3};
  int x = 0;
  P a{arr};
  P b{&x};
  static int st;
  P c{&st};
  P g{&gl};
  (void)c;
}
