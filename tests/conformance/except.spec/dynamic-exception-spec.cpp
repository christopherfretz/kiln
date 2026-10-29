// [except.spec] / [diff.cpp14.except]: dynamic exception specifications other
// than throw() were removed in C++17 (clang: -Wdynamic-exception-spec, an
// error by default); throw() remains accepted
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:10 error "ISO C++17 does not allow dynamic exception specifications"
//@ diag: 10:10 note "use 'noexcept(false)' instead"
//@ diag: 11:10 error "ISO C++17 does not allow dynamic exception specifications"
//@ diag: 11:10 note "use 'noexcept(false)' instead"
void a() throw(int);
void b() throw(int, char);
void c() throw();
