// [except.spec]/4 (C++14 [except.spec]/3): redeclarations with dynamic
// exception specifications must list the same set of adjusted types; a
// plain redeclaration of a special member keeps the earlier list.  WP-1052.
//@ std: c++14
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 29:26 error "exception specification in declaration does not match previous declaration"
//@ diag: 29:6 note "previous declaration is here"
//@ diag: 31:16 error "exception specification in declaration does not match previous declaration"
//@ diag: 31:6 note "previous declaration is here"
//@ diag: 32:26 error "'d' is missing exception specification 'throw(E1)'"
//@ diag: 32:6 note "previous declaration is here"
//@ diag: 33:32 error "exception specification in declaration does not match previous declaration"
//@ diag: 33:6 note "previous declaration is here"
//@ diag: 34:26 error "exception specification in declaration does not match previous declaration"
//@ diag: 34:6 note "previous declaration is here"
//@ diag: 39:4 error "'S' is missing exception specification 'throw()'"
//@ diag: 38:12 note "previous declaration is here"
//@ diag: 40:4 error "exception specification in declaration does not match previous declaration"
//@ diag: 38:25 note "previous declaration is here"
//@ diag: 41:4 error "'S' is missing exception specification 'noexcept'"
//@ diag: 38:43 note "previous declaration is here"
//@ diag: 42:4 error "exception specification in declaration does not match previous declaration"
//@ diag: 38:61 note "previous declaration is here"
//@ diag: 43:4 error "exception specification in declaration does not match previous declaration"
//@ diag: 38:70 note "previous declaration is here"
struct E1 {}; struct E2 {};
void a() throw(E1); void a() throw(E2);
void b() throw(E1, E2); void b() throw(E2, E1, const E1);
void c(); void c() throw(E1);
void d() throw(E1); void d();
void e() noexcept(false); void e() throw(E1);
void f() throw(E1); void f() throw(E1, E2);
typedef int I; void g() throw(int); void g() throw(I);
namespace ns {
struct E1 {}; struct E2 {};
struct S { S() throw(); S(int) throw(E1); S(char) noexcept; S(long); ~S() throw(E1); S& operator=(const S&) throw(E1); };
S::S() {}
S::S(int) throw(E2) {}
S::S(char) {}
S::S(long) throw(E1) {}
S::~S() throw(E1, E2) {}
S& S::operator=(const S&) throw(E1) { return *this; }
}
