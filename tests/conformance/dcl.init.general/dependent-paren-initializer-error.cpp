// [dcl.init.general]/16.9: a scalar initialized by a dependent parenthesized list of more than one
// expression is diagnosed at instantiation
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:37 error "excess elements in scalar initializer"
//@ diag: 8:9 note "in instantiation of function template specialization 'g<int>' requested here"
template <class T> int g(T t) { int x(t, t); return x; }
int y = g(1);
