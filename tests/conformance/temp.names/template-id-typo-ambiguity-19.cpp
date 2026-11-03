// [temp.names]/3: clang 19's delayed typo correction of an undeclared
// template-id (spec/sema.md "Undeclared template-ids"): an invalid nearest
// candidate gives way to the next distance (valeu, abcdefgY); a function
// is valid where only the name is checked (a return); a lone value is
// corrected in a return, condition or discarded expression but not as a
// callee; a type, or a value declared after the use, is no correction.
//@ requires: clang<22
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 32:58 error "use of undeclared identifier 'valeu'; did you mean 'vala'?"
//@ diag: 32:20 note "'vala' declared here"
//@ diag: 33:65 error "use of undeclared identifier 'abcdefgY'; did you mean 'abcdefxy'?"
//@ diag: 33:20 note "'abcdefxy' declared here"
//@ diag: 34:46 error "use of undeclared identifier 'abcdefgY'; did you mean 'abcdefxy'?"
//@ diag: 34:20 note "'abcdefxy' declared here"
//@ diag: 35:56 error "use of undeclared identifier 'valee'; did you mean 'vale'?"
//@ diag: 35:20 note "'vale' declared here"
//@ diag: 36:39 error "use of undeclared identifier 'whilee'; did you mean 'whilAe'?"
//@ diag: 36:20 note "'whilAe' declared here"
//@ diag: 37:39 error "use of undeclared identifier 'whilee'; did you mean 'whilea'?"
//@ diag: 37:20 note "'whilea' declared here"
//@ diag: 38:43 error "use of undeclared identifier 'valc'; did you mean 'vala'?"
//@ diag: 38:20 note "'vala' declared here"
//@ diag: 39:37 error "use of undeclared identifier 'valc'; did you mean 'vala'?"
//@ diag: 39:20 note "'vala' declared here"
//@ diag: 40:42 error "use of undeclared identifier 'valc'; did you mean 'vala'?"
//@ diag: 40:21 note "'vala' declared here"
//@ diag: 41:44 error "use of undeclared identifier 'valc'"
//@ diag: 42:47 error "use of undeclared identifier 'Wx'"
//@ diag: 43:47 error "use of undeclared identifier 'valc'"
namespace n1 { int vala; int vale(int); int c() { return valeu<int> + 1; } }
namespace n2 { int abcdefxy; int abcdefgh(int); int c(){ return abcdefgY<int> + 1; } }
namespace n3 { int abcdefxy; int c(){ return abcdefgY<int> + 1; } }
namespace n4 { int vale(int); int (*c())(int) { return valee<int>; } }
namespace n6 { int whilAe; void f() { whilee<int> + 1; } }
namespace n7 { int whilea; void f() { whilee<int> + 1; } }
namespace n8 { int vala; int c() { return valc<int>; } }
namespace n9 { int vala; void c() { valc<int>; } }
namespace n10 { int vala; void c() { if (valc<int>) {} } }
namespace n11 { int vala; int c() { return valc<int>(1); } }
namespace n12 { struct W {}; int c() { return Wx<int> + 1; } }
namespace n13 { struct W {}; int c() { return valc<int> + 1; } int vala; }
