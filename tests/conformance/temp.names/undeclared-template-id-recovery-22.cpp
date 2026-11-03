// [temp.names]/3, [dcl.spec.auto], [stmt.pre], [expr.ref], [dcl.fct.default]:
// under clang 22 an uncalled template-id corrected to the function it
// qualifies stands for that function's overload set: a placeholder type
// does not deduce from it ("address of overloaded function ... does not
// match"), a condition resolves its one candidate and warns that its
// address is always true, a member access on it asks for a call, and a
// default argument is converted to the parameter type.
//@ expect: compile-fail
//@ requires: clang>=22
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 58:22 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 52:5 note "'::val' declared here"
//@ diag: 58:22 error "address of overloaded function 'val' does not match required type 'int (int)'"
//@ diag: 52:5 note "candidate function"
//@ diag: 59:32 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 52:5 note "'::val' declared here"
//@ diag: 59:32 error "address of overloaded function 'val' does not match required type 'int (int)'"
//@ diag: 52:5 note "candidate function"
//@ diag: 60:23 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 52:5 note "'::val' declared here"
//@ diag: 60:23 error "address of overloaded function 'val' does not match required type 'int (int)'"
//@ diag: 52:5 note "candidate function"
//@ diag: 61:23 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 52:5 note "'::val' declared here"
//@ diag: 61:23 error "address of overloaded function 'val' does not match required type 'int (int)'"
//@ diag: 52:5 note "candidate function"
//@ diag: 64:17 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 52:5 note "'::val' declared here"
//@ diag: 64:17 warning "address of function '::val<int>' will always evaluate to 'true'"
//@ diag: 64:17 note "prefix with the address-of operator to silence this warning"
//@ diag: 65:23 error "'nget' does not name a template but is followed by template arguments"
//@ diag: 55:17 note "non-template declaration found by name lookup"
//@ diag: 68:19 error "use of undeclared identifier 'get'; did you mean '::get'?"
//@ diag: 54:3 note "'::get' declared here"
//@ diag: 68:19 error "base of member reference is a function; perhaps you meant to call it with no arguments?"
//@ diag: 69:19 error "use of undeclared identifier 'get'; did you mean '::get'?"
//@ diag: 54:3 note "'::get' declared here"
//@ diag: 69:27 error "member reference base type 'S ()' is not a structure or union"
//@ diag: 70:22 error "'nget' does not name a template but is followed by template arguments"
//@ diag: 55:17 note "non-template declaration found by name lookup"
//@ diag: 73:25 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 52:5 note "'::val' declared here"
//@ diag: 73:25 error "address of overloaded function 'val' does not match required type 'int (int)'"
//@ diag: 52:5 note "candidate function"
//@ diag: 73:15 note "passing argument to parameter 'p' here"
//@ diag: 74:17 error "use of undeclared identifier 'val'; did you mean '::val'?"
//@ diag: 52:5 note "'::val' declared here"
//@ diag: 74:17 error "address of overloaded function 'val' does not match required type 'int'"
//@ diag: 52:5 note "candidate function"
//@ diag: 74:13 note "passing argument to parameter 'x' here"
int val(int);
struct S { int m; int f(); };
S get();
namespace N { S nget(); }

// An undeduced type: the recovered overload set does not deduce.
void a1() { auto x = val<int>; }
void a2() { decltype(auto) y = val<int>; }
void a3() { auto *p = val<int>; }
void a4() { auto &r = val<int>; }

// A condition: the one candidate is resolved, then converted to bool.
void c1() { if (val<int>) {} }
void c2() { while (N::nget<int>) {} }

// Member access on the recovered overload set.
void m1() { (void)get<int>.m; }
void m2() { (void)get<int>->m; }
void m3() { (void)N::nget<int>.f(); }

// A default argument.
void d1(int (*p)(int) = val<int>);
void d2(int x = val<int>);
