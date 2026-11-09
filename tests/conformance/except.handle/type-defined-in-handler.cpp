// [except.pre]/1, [dcl.type.general]/3: an exception-declaration and a
// dynamic exception specification take type-ids, which may not define a
// class. clang keeps only the valid types of a throw() list, so one whose
// only type is rejected becomes throw() and draws no C++17 error.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:34 error "'S1' cannot be defined in a type specifier"
//@ diag: 12:38 error "'E2' cannot be defined in a type specifier"
//@ diag: 13:24 error "'S3' cannot be defined in a type specifier"
void f1() { try {} catch (struct S1 {} s) {} }
void f2() { try {} catch (const enum E2 { e } &) {} }
void f3() throw(struct S3 {});
