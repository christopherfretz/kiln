// [temp.names]/3: clang 22's typo correction of an undeclared template-id
// (spec/sema.md "Undeclared template-ids") when the nearest candidates are
// one value and one function: a tie is no correction, whatever the
// initializer (clang 19 picks the one that initializes:
// template-id-typo-tie-19.cpp).
//@ requires: clang>=22
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 19:57 error "use of undeclared identifier 'valc'"
//@ diag: 20:57 error "use of undeclared identifier 'valc'"
//@ diag: 21:58 error "use of undeclared identifier 'valc'"
//@ diag: 22:69 error "use of undeclared identifier 'valc'"
//@ diag: 23:49 error "use of undeclared identifier 'valc'"
//@ diag: 24:50 error "use of undeclared identifier 'valc'"
//@ diag: 25:50 error "use of undeclared identifier 'valc'"
//@ diag: 26:72 error "use of undeclared identifier 'valc'"
//@ diag: 27:55 error "use of undeclared identifier 'valc'"
namespace n1 { int vala; int valb(int); int (*x)(int) = valc<int>; }
namespace n2 { int valb(int); int vala; int (*x)(int) = valc<int>; }
namespace n3 { int *vala; int valb(int); int (&x)(int) = valc<int>; }
namespace n4 { int vala; int valb(int); void f() { int (*x)(int) = (valc<int>); } }
namespace n5 { int vala; int valb(int); int x = valc<int>; }
namespace n6 { int vala; int valb(int); bool x = valc<int>; }
namespace n7 { int vala; int valb(int); auto x = valc<int>; }
namespace n8 { int vala; int valb(int); int valz(int); int (*x)(int) = valc<int>; }
namespace n9 { int vala; int valb(int); int (*x)(int){valc<int>}; }
