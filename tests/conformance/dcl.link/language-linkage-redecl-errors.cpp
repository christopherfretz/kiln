// [dcl.link]/6: a redeclaration of a function or variable cannot change
// its language linkage. A declaration in an `extern "C"` specification
// of an entity first declared with C++ language linkage, or one in an
// `extern "C++"` specification of an entity first declared with C
// language linkage, is an error (clang's
// `haveIncompatibleLanguageLinkages`, `err_different_language_linkage`).
// A redeclaration outside any linkage specification keeps the first
// declaration's linkage; an entity with internal linkage has none; a
// different overload is not a redeclaration.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:19 error "declaration of 'f' has a different language linkage"
//@ diag: 34:17 note "previous declaration is here"
//@ diag: 37:17 error "declaration of 'g' has a different language linkage"
//@ diag: 36:19 note "previous declaration is here"
//@ diag: 39:18 error "declaration of 'v' has a different language linkage"
//@ diag: 38:16 note "previous declaration is here"
//@ diag: 41:17 error "declaration of 'k' has a different language linkage"
//@ diag: 40:6 note "previous declaration is here"
//@ diag: 43:21 error "declaration of 'a' has a different language linkage"
//@ diag: 42:19 note "previous declaration is here"
//@ diag: 46:17 error "declaration of 'nest' has a different language linkage"
//@ diag: 45:6 note "previous declaration is here"
//@ diag: 49:19 error "declaration of 'tdf' has a different language linkage"
//@ diag: 48:15 note "previous declaration is here"
//@ diag: 51:19 error "declaration of 'd1' has a different language linkage"
//@ diag: 50:17 note "previous definition is here"
//@ diag: 53:20 error "declaration of 'w' has a different language linkage"
//@ diag: 52:16 note "previous declaration is here"
//@ diag: 55:27 error "declaration of 'cv' has a different language linkage"
//@ diag: 54:16 note "previous declaration is here"
//@ diag: 57:31 error "declaration of 'nsf' has a different language linkage"
//@ diag: 56:20 note "previous declaration is here"
extern "C" void f();
extern "C++" void f();
extern "C++" void g();
extern "C" void g();
extern "C" int v;
extern "C++" int v;
void k();
extern "C" void k();
extern "C" { void a(); }
extern "C++" { void a(); }
extern "C" { extern "C++" void nest(); }
void nest();
extern "C" void nest();
typedef void T1();
extern "C" T1 tdf;
extern "C++" void tdf();
extern "C" void d1() {}
extern "C++" void d1() {}
extern "C" int w;
extern "C++" { int w; }
extern "C" int cv;
extern "C++" { extern int cv; }
namespace N { void nsf(); }
namespace N { extern "C" void nsf(); }

// Valid: no linkage specification, internal linkage, other overloads.
extern "C" { void h(); }
void h();
static void s1();
extern "C" void s1();
extern "C" void ov(int);
extern "C++" void ov(double);
extern "C" void fr();
struct F { friend void fr(); };
