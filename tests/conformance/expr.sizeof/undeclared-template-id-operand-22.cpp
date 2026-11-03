// [expr.sizeof]/1, [expr.alignof]/1, [temp.names]/3: under clang 22 an
// operand naming an uncalled template-id corrected to the function it
// qualifies is resolved to that function, which sizeof and alignof reject;
// a member access on it asks for a call.
//@ expect: compile-fail
//@ requires: clang>=22
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 35:26 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 28:5 note "'::val' declared here"
//@ diag: 35:25 error "invalid application of 'sizeof' to a function type"
//@ diag: 36:26 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 28:5 note "'::val' declared here"
//@ diag: 36:26 error "invalid application of 'sizeof' to a function type"
//@ diag: 37:27 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 28:5 note "'::val' declared here"
//@ diag: 37:19 warning "'alignof' applied to an expression is a GNU extension"
//@ diag: 37:26 error "invalid application of 'alignof' to a function type"
//@ diag: 38:26 error "use of undeclared identifier 'get'; did you mean '::get'?"
//@ diag: 30:3 note "'::get' declared here"
//@ diag: 38:25 error "invalid application of 'sizeof' to a function type"
//@ diag: 39:26 error "use of undeclared identifier 'get'; did you mean '::get'?"
//@ diag: 30:3 note "'::get' declared here"
//@ diag: 39:26 error "base of member reference is a function; perhaps you meant to call it with no arguments?"
//@ diag: 40:35 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 28:5 note "'::val' declared here"
//@ diag: 40:19 error "invalid application of 'sizeof' to a function type"
int val(int);
struct S { int m; };
S get();

// An operand naming an undeclared template-id recovered as the overload
// set of the function it qualifies: the operand is resolved to the
// function, which sizeof and alignof reject.
void s1() { (void)sizeof(val<int>); }
void s2() { (void)sizeof val<int>; }
void s3() { (void)alignof(val<int>); }
void s4() { (void)sizeof(get<int>); }
void s5() { (void)sizeof(get<int>.m); }
void s6() { (void)sizeof(decltype(val<int>)); }
