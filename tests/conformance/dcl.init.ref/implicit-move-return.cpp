// [class.copy.elision]/3, [expr.prim.id.unqual]/3 (P2266R3): a returned
// local or parameter is an xvalue, so it does not bind to a returned
// non-const lvalue reference ([dcl.init.ref]/5.2); a const one binds it
// and dangles (-Wreturn-stack-address).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:25 error "non-const lvalue reference to type 'int' cannot bind to a temporary of type 'int'"
//@ diag: 15:31 error "non-const lvalue reference to type 'int' cannot bind to a temporary of type 'int'"
//@ diag: 16:27 error "non-const lvalue reference to type 'int' cannot bind to a temporary of type 'int'"
//@ diag: 17:40 error "volatile lvalue reference to type 'volatile int' cannot bind to a temporary of type 'int'"
//@ diag: 18:37 warning "reference to stack memory associated with local variable 'x' returned [-Wreturn-stack-address]"
//@ diag: 19:32 warning "reference to stack memory associated with local variable 'x' returned [-Wreturn-stack-address]"
//@ diag: 21:21 error "non-const lvalue reference to type 'M' cannot bind to a temporary of type 'M'"
int& f1(int x) { return x; }
int& f2() { int x = 0; return x; }
int& f3(int&& x) { return x; }
volatile int& f4() { int x = 0; return x; }
const int& f5() { int x = 0; return x; }
int&& f6() { int x = 0; return x; }
struct M { int v; };
M& f7(M m) { return m; }
