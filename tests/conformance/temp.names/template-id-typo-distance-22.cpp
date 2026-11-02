// [temp.names]/3: clang 22's typo correction of an undeclared template-id
// (spec/sema.md "Undeclared template-ids") bounds the distance: a name
// under three characters is only corrected to itself qualified; a longer
// one to a name at most (len+2)/3 edits away and a third of its length
// once rounded; two candidates at the least distance (abcdefhg) are no
// correction.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 33:19 error "use of undeclared identifier 'ab'; did you mean '::ab'?"
//@ diag: 31:5 note "'::ab' declared here"
//@ diag: 33:27 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 34:19 error "use of undeclared identifier 'abc'; did you mean 'ab'?"
//@ diag: 31:5 note "'ab' declared here"
//@ diag: 34:28 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 35:19 error "use of undeclared identifier 'abcdeg'; did you mean 'abcdef'?"
//@ diag: 31:32 note "'abcdef' declared here"
//@ diag: 35:31 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 36:19 error "use of undeclared identifier 'nab'; did you mean 'ab'?"
//@ diag: 31:5 note "'ab' declared here"
//@ diag: 36:28 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 37:19 error "use of undeclared identifier 'nabce'"
//@ diag: 38:19 error "use of undeclared identifier 'nabcdefgi'; did you mean 'abcdefgh'?"
//@ diag: 31:49 note "'abcdefgh' declared here"
//@ diag: 38:34 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 39:19 error "use of undeclared identifier 'abcdefhg'"
//@ diag: 40:19 error "use of undeclared identifier 'abcdfegh'; did you mean 'abcdefgh'?"
//@ diag: 31:49 note "'abcdefgh' declared here"
//@ diag: 40:33 error "arithmetic on a pointer to the function type 'int (int)'"
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
