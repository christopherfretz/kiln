// [dcl.fct.default]/9: a parameter shall not appear as a
// potentially-evaluated expression in a default argument; it is in scope
// there ([basic.scope.param]), its own default argument included
// ([basic.scope.pdecl]/1). Unevaluated operands may name it.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:24 error "default argument references parameter 'a'"
//@ diag: 15:17 error "default argument references parameter 'a'"
//@ diag: 16:25 error "default argument references parameter 'a'"
//@ diag: 18:25 error "default argument references parameter 'a'"
//@ diag: 21:28 error "default argument references parameter 'b'"
int a;
void ok(int x = a);
void f1(int a, int b = a);
void f2(int a = a);
void f4(int a, int b = (a + 1));
struct S {
  void m(int a, int b = a);
  void n(int a, int b = sizeof a, int c = decltype(a)(1));
};
void f5(int b, int c = 1 + b * 2);
void f6(int a, int b = sizeof(a), int c = noexcept(a + 1));
