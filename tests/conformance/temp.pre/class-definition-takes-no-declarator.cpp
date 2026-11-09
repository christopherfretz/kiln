// [temp.pre]/5: in a template-declaration that defines a class, the
// init-declarator-list shall be empty. clang (ParseClassSpecifier) demands
// the ';' after the class body and then parses the rest as a new
// declaration.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:30 error "expected ';' after struct"
//@ diag: 11:31 error "a type specifier is required for all declarations"
//@ diag: 13:30 error "expected ';' after struct"
template<class T> struct A {} a;
template<class T> struct C { } ;
template<class T> struct D {}
int x;
