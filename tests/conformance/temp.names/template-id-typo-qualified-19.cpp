// [temp.names]/3: clang 19's delayed typo correction of an undeclared
// template-id (spec/sema.md "Undeclared template-ids"): a function found by
// qualified lookup in a namespace or class is rebuilt qualified and, as an
// operand, invalid, so nothing is corrected.
//@ requires: clang<22
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 18:19 error "use of undeclared identifier 'xy'"
//@ diag: 19:19 error "use of undeclared identifier 'f'"
//@ diag: 20:19 error "use of undeclared identifier 'abcd'"
//@ diag: 21:19 error "use of undeclared identifier 'abcdefgi'"
//@ diag: 22:19 error "use of undeclared identifier 'rfuncc'"
//@ diag: 23:19 error "use of undeclared identifier 'rg'"
namespace A { namespace B { int xy(int); int abcd(int); int abcdefgh(int); } }
int f(int);
struct R { static int rfunc(int); static int rf(int); };
int x1() { return xy<int> + 1; }
int x2() { return f<int> + 1; }
int x3() { return abcd<int> + 1; }
int x4() { return abcdefgi<int> + 1; }
int x5() { return rfuncc<int> + 1; }
int x6() { return rg<int> + 1; }
