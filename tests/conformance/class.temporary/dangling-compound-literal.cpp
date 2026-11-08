// [class.temporary]/6, GNU compound literals in C++ (WP-876): a compound
// literal is a prvalue temporary, so pointing at its array or binding a
// reference to it from a returned expression, a pointer member or a local
// pointer dangles; a reference bound directly to one is extended.
//@ strict-diags
//@ expect: compile-fail
//@ diag: 14:35 warning "reference member 'p' binds to a temporary object whose lifetime is shorter than the lifetime of the constructed object"
//@ diag: 14:18 note "reference member declared here"
//@ diag: 16:12 warning "temporary whose address is used as value of local variable 'h' will be destroyed at the end of the full-expression"
//@ diag: 18:13 warning "temporary whose address is used as value of local variable 'ok' will be destroyed at the end of the full-expression"
//@ diag: 21:20 warning "returning address of local temporary object"
//@ diag: 22:26 error "returning reference to local temporary object"

struct S2 { int *p; S2(int x) : p((int[]){x}) {} };
void f() {
  int *h = (int[]){1, 2};
  const int &i = (const int){5};
  int *ok = (int[]){3};
  (void)h; (void)i; (void)ok;
}
int *r1() { return (int[]){1, 2}; }
const int &r2() { return (const int){3}; }
