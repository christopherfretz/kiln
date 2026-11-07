// [dcl.init.string]/2, [dcl.init.general]/16.5: with a dependent element
// type, a string literal too long for the array, or a non-literal scalar
// initializing an array, is diagnosed in each instantiation, not in the
// template. WP-681.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:39 error "initializer-string for char array is too long, array size is 2 but initializer has size 4 (including the null terminating character)"
//@ diag: 14:9 note "in instantiation of function template specialization 'f<char>' requested here"
//@ diag: 13:48 error "array initializer must be an initializer list or string literal"
//@ diag: 13:32 error "array initializer must be an initializer list"
//@ diag: 15:9 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 13:48 error "array initializer must be an initializer list"
template <class T> int f() { T a[2] = "abc"; T b[2] = 1; return a[0] + b[0]; }
int x = f<char>();
int y = f<int>();
