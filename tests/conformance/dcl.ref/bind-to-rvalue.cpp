// [dcl.init.ref]/5: a non-const lvalue reference cannot bind to an rvalue
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:6 error "non-const lvalue reference to type 'int' cannot bind to a temporary of type 'int'"
int &r = 5;
