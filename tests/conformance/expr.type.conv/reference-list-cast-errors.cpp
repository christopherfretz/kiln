// [dcl.init.list]/3.10, [dcl.init.ref]/5: `R{...}` for a non-const lvalue
// reference type `R` binds no temporary, and the element of a temporary
// is not narrowed.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 12:9 error "non-const lvalue reference to type 'int' cannot bind to an initializer list temporary"
//@ diag: 13:15 error "non-constant-expression cannot be narrowed from type 'int' to 'double' in initializer list"
//@ diag: 13:15 note "insert an explicit cast to silence this issue"
using LR = int &;
using CR = const double &;
int a = LR{5};
double b = CR{a};
