// [expr.sizeof]/1, [dcl.type.decltype], [temp.names]/3: under clang 19 an
// uncalled template-id in an unevaluated operand is a delayed typo whose
// correction fails where the rebuilt sizeof or member access would be
// invalid; a decltype operand is corrected as soon as it is parsed, before
// the enclosing sizeof rejects the function type.
//@ expect: compile-fail
//@ requires: clang<22
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 26:26 error "use of undeclared identifier 'val'"
//@ diag: 27:26 error "use of undeclared identifier 'val'"
//@ diag: 28:19 warning "'alignof' applied to an expression is a GNU extension"
//@ diag: 28:27 error "use of undeclared identifier 'val'"
//@ diag: 29:26 error "use of undeclared identifier 'get'"
//@ diag: 30:26 error "use of undeclared identifier 'get'"
//@ diag: 31:35 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 19:5 note "'::val' declared here"
//@ diag: 31:19 error "invalid application of 'sizeof' to a function type"
int val(int);
struct S { int m; };
S get();

// An operand naming an undeclared template-id: the correction to the
// function it qualifies fails where sizeof, alignof or the member access
// would reject it, and stands for the decltype operand.
void s1() { (void)sizeof(val<int>); }
void s2() { (void)sizeof val<int>; }
void s3() { (void)alignof(val<int>); }
void s4() { (void)sizeof(get<int>); }
void s5() { (void)sizeof(get<int>.m); }
void s6() { (void)sizeof(decltype(val<int>)); }
