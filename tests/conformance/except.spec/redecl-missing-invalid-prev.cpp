// [except.spec]/5: the missing-specification check compares a
// redeclaration with a valid previous declaration only (clang
// `CheckEquivalentExceptionSpec` is skipped for an invalid one). A
// function declaration given a pure-specifier is invalid, so its
// redeclarations are invalid without a diagnostic (WP-288); an invalid
// in-class redeclaration defers to the first member declaration (WP-279).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:6 error "illegal initializer (only variables can be initialized)"
//@ diag: 27:36 error "class member cannot be redeclared"
//@ diag: 27:17 note "previous declaration is here"
//@ diag: 28:9 error "'f' is missing exception specification 'noexcept'"
//@ diag: 27:17 note "previous declaration is here"
//@ diag: 31:6 error "'a' is missing exception specification 'noexcept'"
//@ diag: 30:6 note "previous declaration is here"
//@ diag: 34:9 error "'g' is missing exception specification 'noexcept'"
//@ diag: 33:20 note "previous declaration is here"
//@ diag: 35:9 error "'g' is missing exception specification 'noexcept'"
//@ diag: 33:20 note "previous declaration is here"

void e() noexcept = 0;
void e();
void e() noexcept;
static_assert(noexcept(e()));

// The in-class duplicate is invalid; the definition matches the first.
struct S { void f() noexcept; void f() noexcept; };
void S::f() {}

void a() noexcept;
void a();

namespace n { void g() noexcept; }
void n::g();
void n::g() {}
