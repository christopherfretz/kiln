// [temp.names]/3: clang 19's delayed typo correction of an undeclared
// template-id (spec/sema.md "Undeclared template-ids") when the nearest
// candidates are one value and one function: copy-initialization
// rebuilds the initializer once per candidate, so a value that cannot
// initialize the declared type (a function pointer or reference) leaves
// the function the correction, a function that cannot leaves the value,
// and both valid (bool, auto) or a second function is no correction;
// direct-list-initialization checks the name only.
//@ requires: clang<22
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 27:57 error "use of undeclared identifier 'valc'; did you mean 'valb'?"
//@ diag: 27:30 note "'valb' declared here"
//@ diag: 28:57 error "use of undeclared identifier 'valc'; did you mean 'valb'?"
//@ diag: 28:20 note "'valb' declared here"
//@ diag: 29:58 error "use of undeclared identifier 'valc'; did you mean 'valb'?"
//@ diag: 29:31 note "'valb' declared here"
//@ diag: 30:69 error "use of undeclared identifier 'valc'; did you mean 'valb'?"
//@ diag: 30:30 note "'valb' declared here"
//@ diag: 31:49 error "use of undeclared identifier 'valc'; did you mean 'vala'?"
//@ diag: 31:20 note "'vala' declared here"
//@ diag: 32:50 error "use of undeclared identifier 'valc'"
//@ diag: 33:50 error "use of undeclared identifier 'valc'"
//@ diag: 34:72 error "use of undeclared identifier 'valc'"
//@ diag: 35:55 error "use of undeclared identifier 'valc'"
namespace n1 { int vala; int valb(int); int (*x)(int) = valc<int>; }
namespace n2 { int valb(int); int vala; int (*x)(int) = valc<int>; }
namespace n3 { int *vala; int valb(int); int (&x)(int) = valc<int>; }
namespace n4 { int vala; int valb(int); void f() { int (*x)(int) = (valc<int>); } }
namespace n5 { int vala; int valb(int); int x = valc<int>; }
namespace n6 { int vala; int valb(int); bool x = valc<int>; }
namespace n7 { int vala; int valb(int); auto x = valc<int>; }
namespace n8 { int vala; int valb(int); int valz(int); int (*x)(int) = valc<int>; }
namespace n9 { int vala; int valb(int); int (*x)(int){valc<int>}; }
