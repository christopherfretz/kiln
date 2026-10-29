// [except.spec]/5: if any declaration of a function has a non-throwing
// exception specification, all declarations shall have a compatible one
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:6 error "'a' is missing exception specification 'noexcept'"
//@ diag: 9:6 note "previous declaration is here"
//@ diag: 12:6 error "exception specification in declaration does not match previous declaration"
//@ diag: 11:6 note "previous declaration is here"
void a() noexcept;
void a();
void b();
void b() noexcept;
void c() noexcept(true);
void c() noexcept;
