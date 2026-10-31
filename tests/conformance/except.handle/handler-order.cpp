// [except.handle]/4: a handler for a type that is caught by an earlier
// handler (the same type, or a reference/pointer to a public base) is
// never reached; clang warns (-Wexceptions) at the later handler's type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:32 warning "exception of type 'int &' will be caught by earlier handler"
//@ diag: 19:17 note "for type 'int'"
//@ diag: 20:32 warning "exception of type 'D &' will be caught by earlier handler"
//@ diag: 20:17 note "for type 'B &'"
//@ diag: 21:44 warning "exception of type 'const D *' will be caught by earlier handler"
//@ diag: 21:23 note "for type 'const B *'"
//@ diag: 23:30 warning "exception of type 'D' will be caught by earlier handler"
//@ diag: 23:17 note "for type 'B'"
//@ diag: 26:3 error "use of undeclared identifier 'stop'"
struct B {};
struct D : B {};
struct P : private B {};
void f() {
  try {} catch (int) {} catch (int &) {}
  try {} catch (B &) {} catch (D &) {}
  try {} catch (const B *) {} catch (const D *) {}
  try {} catch (B &) {} catch (P &) {}
  try {} catch (B) {} catch (D) {}
}
void g() {
  stop;
}
