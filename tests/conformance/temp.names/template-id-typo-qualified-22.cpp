// [temp.names]/3: clang 22's typo correction of an undeclared template-id
// (spec/sema.md "Undeclared template-ids") qualifies a name ordinary
// lookup does not find by a namespace or class declaring it, each
// qualifier component costing more than a character edit; the qualified
// correction obeys the same third-of-the-length bound (abcd, rfuncc).
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 25:19 error "use of undeclared identifier 'xy'; did you mean 'A::B::xy'?"
//@ diag: 22:33 note "'A::B::xy' declared here"
//@ diag: 25:27 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 26:19 error "use of undeclared identifier 'f'; did you mean '::f'?"
//@ diag: 23:5 note "'::f' declared here"
//@ diag: 26:26 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 27:19 error "use of undeclared identifier 'abcd'"
//@ diag: 28:19 error "use of undeclared identifier 'abcdefgi'"
//@ diag: 29:19 error "use of undeclared identifier 'rfuncc'; did you mean 'R::rfunc'?"
//@ diag: 24:23 note "'R::rfunc' declared here"
//@ diag: 29:31 error "arithmetic on a pointer to the function type 'int (int)'"
//@ diag: 30:19 error "use of undeclared identifier 'rg'"
namespace A { namespace B { int xy(int); int abcd(int); int abcdefgh(int); } }
int f(int);
struct R { static int rfunc(int); static int rf(int); };
int x1() { return xy<int> + 1; }
int x2() { return f<int> + 1; }
int x3() { return abcd<int> + 1; }
int x4() { return abcdefgi<int> + 1; }
int x5() { return rfuncc<int> + 1; }
int x6() { return rg<int> + 1; }
