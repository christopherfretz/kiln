// [temp.names]/3: clang 19's delayed typo correction of an undeclared
// template-id (spec/sema.md "Undeclared template-ids"): a function, plain
// or found qualified, is never a valid built-in operand, so a name near
// only functions is not corrected at any distance.
//@ requires: clang<22
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 19:19 error "use of undeclared identifier 'ab'"
//@ diag: 20:19 error "use of undeclared identifier 'abc'"
//@ diag: 21:19 error "use of undeclared identifier 'abcdeg'"
//@ diag: 22:19 error "use of undeclared identifier 'nab'"
//@ diag: 23:19 error "use of undeclared identifier 'nabce'"
//@ diag: 24:19 error "use of undeclared identifier 'nabcdefgi'"
//@ diag: 25:19 error "use of undeclared identifier 'abcdefhg'"
//@ diag: 26:19 error "use of undeclared identifier 'abcdfegh'"
int ab(int); int abc(int); int abcdef(int); int abcdefgh(int);
namespace N { int nab(int); int nabcd(int); int nabcdefgh(int); }
int x1() { return ab<int> + 1; }
int x2() { return abc<int> + 1; }
int x3() { return abcdeg<int> + 1; }
int x4() { return nab<int> + 1; }
int x5() { return nabce<int> + 1; }
int x6() { return nabcdefgi<int> + 1; }
int x7() { return abcdefhg<int> + 1; }
int x8() { return abcdfegh<int> + 1; }
