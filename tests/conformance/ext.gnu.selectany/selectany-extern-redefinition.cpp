// An `extern` declaration with its own `selectany` is a definition
// (clang `VarDecl::isThisDeclarationADefinition`), so a later definition
// with an initializer redefines it ([basic.def.odr]/1), and a const one
// is default-initialized ([dcl.init.general]/8) (WP-1745).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:5 error "redefinition of 'd'"
//@ diag: 10:39 note "previous definition is here"
//@ diag: 12:45 error "default initialization of an object of const type 'const int'"
extern __attribute__((selectany)) int d;
int d = 4;
extern __attribute__((selectany)) const int k;
