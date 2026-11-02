// [dcl.fct.default]: an erroneous default argument makes its parameter and
// function invalid (clang's `ActOnParamDefaultArgumentError`). The invalid
// declaration redeclares nothing, a redeclaration of it is invalid without a
// diagnostic (no missing exception specification, [except.spec]/5), and a
// call through only invalid declarations is resolved against the last one.
// A default argument of a member declared in its class is parsed in the
// complete-class context, after the member is declared, and leaves it valid
// (WP-288).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:16 error "use of undeclared identifier 'y'"
//@ diag: 28:27 error "use of undeclared identifier 'q'"
//@ diag: 30:12 error "cannot initialize a parameter of type 'int' with an lvalue of type 'const char[2]'"
//@ diag: 30:12 note "passing argument to parameter 'x' here"
//@ diag: 34:16 error "use of undeclared identifier 'w'"
//@ diag: 35:6 error "exception specification in declaration does not match previous declaration"
//@ diag: 33:6 note "previous declaration is here"
//@ diag: 37:27 error "use of undeclared identifier 'zz'"
//@ diag: 38:9 error "'m' is missing exception specification 'noexcept'"
//@ diag: 37:17 note "previous declaration is here"
//@ diag: 41:3 error "no matching function for call to 'g'"
//@ diag: 27:6 note "candidate function not viable: requires single argument 'x', but no arguments were provided"
//@ diag: 43:3 error "no matching function for call to 'h'"
//@ diag: 29:6 note "candidate function not viable: requires 2 arguments, but 0 were provided"

void g(int x = y) noexcept;
void g(int x);
void h(int x = 1, int z = q) noexcept;
void h(int x, int z) {}
void d(int x = "s") noexcept;
void d(int x);

void k(int) noexcept;
void k(int x = w);
void k(int x) noexcept(false);

struct S { void m(int a = zz) noexcept; };
void S::m(int a) {}

void use() {
  g();
  g(1);
  h();
  h(1, 2);
  k(2);
}
