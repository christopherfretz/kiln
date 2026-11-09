// [basic.start.main]/3: a program that declares an entity named `main` with
// C language linkage, in any namespace, is ill-formed. clang 22 reports a
// variable at its decl-specifiers; clang 19 only the global one (WP-1040).
//@ std: c++26
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:26 error "main cannot be declared as a variable with C language linkage"
//@ diag: 12:28 error "main cannot be declared as a variable with C language linkage"
//@ diag: 13:12 error "main cannot be declared as a variable with C language linkage"
namespace N { extern "C" int main; }
namespace M { extern "C" { int main = 0; } }
extern "C" int main;
