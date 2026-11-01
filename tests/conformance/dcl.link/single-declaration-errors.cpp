// [dcl.link]/8: only the declaration directly contained in an
// extern "C" without braces is a declaration; inside braces `int b;` is a
// definition. A declaration that becomes a definition through its
// initializer still redefines an earlier definition (clang
// `AddInitializerToDecl`), with or without `extern`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:16 error "redefinition of 'a'"
//@ diag: 17:16 note "previous definition is here"
//@ diag: 20:5 error "redefinition of 'b'"
//@ diag: 19:18 note "previous definition is here"
//@ diag: 21:12 warning "'extern' variable has an initializer"
//@ diag: 22:12 error "redefinition of 'c'"
//@ diag: 21:12 note "previous definition is here"
//@ diag: 24:12 error "redefinition of 'd'"
//@ diag: 23:5 note "previous definition is here"
extern "C" int a = 1;
extern "C" int a = 2;
extern "C" { int b; }
int b;
extern int c = 1;
extern int c = 2;
int d;
extern int d = 2;
