// [dcl.attr.noreturn]/1: "The first declaration of a function shall
// specify the noreturn attribute if any declaration of that function
// specifies the noreturn attribute." clang `MergeFunctionDecl`
// (`err_attribute_missing_on_first_decl`, noting the declaration just
// before) checks the standard spelling only: the GNU spellings and the
// C11 `_Noreturn` keyword may be added later. C has no such rule
// (tests/c/c23/noreturn-later-declaration.c). WP-1311.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:3 error "'noreturn' attribute does not appear on the first declaration"
//@ diag: 20:6 note "previous declaration is here"
//@ diag: 24:10 error "'noreturn' attribute does not appear on the first declaration"
//@ diag: 23:6 note "previous declaration is here"
//@ diag: 29:3 error "'noreturn' attribute does not appear on the first declaration"
//@ diag: 28:32 note "previous declaration is here"
//@ diag: 37:3 error "'noreturn' attribute does not appear on the first declaration"
//@ diag: 36:17 note "previous declaration is here"
//@ diag: 39:22 error "'noreturn' attribute does not appear on the first declaration"
//@ diag: 38:25 note "previous declaration is here"
void f();
[[noreturn]] void f();
void g();
void g();
void g [[noreturn]] ();
[[noreturn]] void h();
[[noreturn]] void h();
void h();
__attribute__((noreturn)) void k();
[[noreturn]] void k();
void m();
[[gnu::noreturn]] void m();
void n();
__attribute__((noreturn)) void n();
void p();
_Noreturn void p();
struct S { void s(); };
[[noreturn]] void S::s() { throw 1; }
template <class T> void t();
template <class T> [[noreturn]] void t();
