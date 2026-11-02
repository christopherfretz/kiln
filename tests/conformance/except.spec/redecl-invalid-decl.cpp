// [except.spec]/5, [basic.scope.scope]/4: an invalid declaration of a
// function is on no redeclaration chain. A later declaration is checked
// against the newest valid corresponding declaration, never an invalid one
// (clang's lookup ignores an invalid declaration unless it is the only one
// left; `MergeFunctionDecl` gives up on an invalid old declaration, leaving
// the new one invalid without a diagnostic) (WP-288).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:1 error "illegal storage class on function"
//@ diag: 26:6 error "'a' is missing exception specification 'noexcept'"
//@ diag: 24:6 note "previous declaration is here"
//@ diag: 29:1 error "illegal storage class on function"
//@ diag: 30:6 error "exception specification in declaration does not match previous declaration"
//@ diag: 28:6 note "previous declaration is here"
//@ diag: 32:1 error "illegal storage class on function"
//@ diag: 37:14 error "use of undeclared identifier 'x'"
//@ diag: 39:6 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 36:5 note "previous declaration is here"
//@ diag: 41:36 error "class member cannot be redeclared"
//@ diag: 41:17 note "previous declaration is here"
//@ diag: 42:9 error "'f' is missing exception specification 'noexcept'"
//@ diag: 41:17 note "previous declaration is here"

void a() noexcept;
mutable void a();
void a();

void b();
mutable void b() noexcept;
void b() noexcept;

mutable void c() noexcept;
void c();
void c() noexcept(false);

int e() noexcept;
void e(int = x) noexcept;
void e(int);
long e();

struct S { void f() noexcept; void f(); };
void S::f() {}
