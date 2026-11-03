// [expr.cond]/4-7, [temp.names]/3: under clang 22 an arm naming an
// uncalled template-id corrected to '::val' is resolved to that function and
// converted to a pointer like any function arm.
//@ expect: compile-fail
//@ requires: clang>=22
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 27:30 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 21:5 note "'::val' declared here"
//@ diag: 28:30 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 21:5 note "'::val' declared here"
//@ diag: 28:41 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 21:5 note "'::val' declared here"
//@ diag: 29:30 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 21:5 note "'::val' declared here"
//@ diag: 30:37 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 21:5 note "'::val' declared here"
//@ diag: 31:35 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 21:5 note "'::val' declared here"
//@ diag: 31:29 error "incompatible operand types ('int' and 'int (*)(int)')"
int val(int);
int other(int);

// An arm naming an undeclared template-id recovered as the overload set
// of '::val': each arm's correction is reported, and the conditional is
// resolved against the other arm.
void c1(bool c) { (void)(c ? val<int> : 0); }
void c2(bool c) { (void)(c ? val<int> : val<int>); }
void c3(bool c) { (void)(c ? val<int> : other); }
int (*c4(bool c))(int) { return c ? val<int> : other; }
void c5(bool c) { int x = c ? 1 : val<int>; }
