// [dcl.enum]/10: scoped enumerators do not convert implicitly
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:5 error "cannot initialize a variable of type 'int' with an rvalue of type 'S'"
enum class S { X };
int i = S::X;
