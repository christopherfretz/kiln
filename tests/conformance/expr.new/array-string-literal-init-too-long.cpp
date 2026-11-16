// [dcl.init.string]/2: a string literal initializing an array new-expression
// must fit the bound, in the braced and the parenthesized (C++20) form, and
// the element type must be compatible with the literal.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 11:32 error "initializer-string for char array is too long, array size is 2 but initializer has size 4 (including the null terminating character)"
//@ diag: 12:32 error "initializer-string for char array is too long, array size is 2 but initializer has size 4 (including the null terminating character)"
//@ diag: 13:29 error "cannot initialize an array element of type 'int' with an lvalue of type 'const char[4]'"

char* f() { return new char[2]{"foo"}; }
char* g() { return new char[2]("foo"); }
int* h() { return new int[]("foo"); }
