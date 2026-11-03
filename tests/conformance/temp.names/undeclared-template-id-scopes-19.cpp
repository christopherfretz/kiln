// [temp.names]/3, [class.mem.general], [dcl.fct.default], [temp.arg.nontype],
// [stmt.pre], [dcl.init.aggr]: under clang 19 the delayed typo of an
// undeclared template-id is corrected where clang 19 corrects it: a default
// member initializer, a default argument, a template argument and each
// element of a braced list are corrected as soon as they are parsed, with
// no initialization filter (a corrected name the initialization then
// rejects still reports "did you mean"; one whose rebuilt expression is
// invalid, as sizeof of a function, does not). A condition declaration is
// filtered by its initialization, as any initializer.
//@ expect: compile-fail
//@ requires: clang<22
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 45:29 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 40:5 note "'::val' declared here"
//@ diag: 46:21 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 40:5 note "'::val' declared here"
//@ diag: 46:21 error "cannot initialize a member subobject of type 'int' with an lvalue of type 'int (int)'"
//@ diag: 49:25 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 40:5 note "'::val' declared here"
//@ diag: 50:17 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 40:5 note "'::val' declared here"
//@ diag: 50:13 error "cannot initialize a parameter of type 'int' with an lvalue of type 'int (int)'"
//@ diag: 50:13 note "passing argument to parameter 'x' here"
//@ diag: 53:10 error "use of undeclared identifier 'val'"
//@ diag: 54:12 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 40:5 note "'::val' declared here"
//@ diag: 57:33 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 40:5 note "'::val' declared here"
//@ diag: 58:25 error "use of undeclared identifier 'val'"
//@ diag: 59:29 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 40:5 note "'::val' declared here"
//@ diag: 62:34 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 40:5 note "'::val' declared here"
//@ diag: 62:44 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 40:5 note "'::val' declared here"
//@ diag: 63:26 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 40:5 note "'::val' declared here"
//@ diag: 63:26 error "cannot initialize an array element of type 'int' with an lvalue of type 'int (int)'"
int val(int);
template <int N> struct A {};
template <class T> struct B {};

// A default member initializer: corrected with no filter.
struct S1 { int (*p)(int) = val<int>; };
struct S2 { int x = val<int>; };

// A default argument: corrected before the conversion.
void d1(int (*p)(int) = val<int>);
void d2(int x = val<int>);

// A template argument (a constant-expression).
A<sizeof(val<int>)> a1;
B<decltype(val<int>)> b1;

// A condition declaration: the initialization is the filter.
void c1() { if (int (*p)(int) = val<int>) {} }
void c2() { if (int x = val<int>) {} }
void c3() { while (auto p = val<int>) {} }

// Braced elements: each corrected as it is parsed.
void e1() { int (*a[2])(int) = { val<int>, val<int> }; }
void e2() { int a[2] = { val<int>, 1 }; }
