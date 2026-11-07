// GCC "Common Function Attributes", alias; [basic.def]/2, [basic.def.odr]/1:
// an `alias` declaration is a definition (clang `hasDefiningAttr`), so a
// later tentative definition `int v;`, a definition with an initializer
// (also with `extern`, which then has no -Wextern-initializer warning:
// clang `AddInitializerToDecl` stops at the redefinition), and a
// definition before the alias are redefinitions; a later `extern "C"`
// redeclaration names the alias's definition in its note.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:5 error "redefinition of 'v'"
//@ diag: 21:12 note "previous definition is here"
//@ diag: 24:5 error "redefinition of 'v1'"
//@ diag: 23:12 note "previous definition is here"
//@ diag: 26:12 error "redefinition of 'w'"
//@ diag: 25:12 note "previous definition is here"
//@ diag: 28:16 error "declaration of 'x' has a different language linkage"
//@ diag: 27:12 note "previous definition is here"
//@ diag: 30:12 error "redefinition of 'u'"
//@ diag: 29:5 note "previous definition is here"
extern "C" int tgt = 0;
extern int v __attribute__((alias("tgt")));
int v;
extern int v1 __attribute__((alias("tgt")));
int v1 = 1;
extern int w __attribute__((alias("tgt")));
extern int w = 2;
extern int x __attribute__((alias("tgt")));
extern "C" int x;
int u;
extern int u __attribute__((alias("tgt")));
extern int ok __attribute__((alias("tgt")));
extern int ok;
