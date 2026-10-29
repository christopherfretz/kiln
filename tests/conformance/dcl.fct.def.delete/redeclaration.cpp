// [dcl.fct.def.delete]/4: a deleted definition is the first declaration;
// [basic.def.odr]/1: it is a definition, so it cannot be redefined; P2573
// deletion messages.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:12 error "deleted definition must be first declaration"
//@ diag: 16:6 note "previous declaration is here"
//@ diag: 19:6 error "redefinition of 'g'"
//@ diag: 18:6 note "previous definition is here"
//@ diag: 24:15 error "deleted definition must be first declaration"
//@ diag: 21:8 note "previous declaration is here"
//@ diag: 26:14 error "call to deleted function 'h': message"
//@ diag: 25:6 note "candidate function has been explicitly deleted"
//@ diag: 28:6 error "redefinition of 'k'"
//@ diag: 27:6 note "previous definition is here"
void f();
void f() = delete;
void g() = delete;
void g() = delete;
struct S {
  void m();
  S(int) = delete;
};
void S::m() = delete;
void h() = delete("message");
void use() { h(); }
void k() = delete;
void k() {}
