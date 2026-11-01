// [dcl.init.general]/16.1, /16.9: a parenthesized braced-init-list does not
// initialize a non-class type, in a functional cast too (clang's
// FK_ParenthesizedListInitForScalar), also at instantiation. Matches clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:9 error "cannot initialize non-class type 'int' with a parenthesized initializer list"
//@ diag: 9:37 error "cannot initialize non-class type 'int' with a parenthesized initializer list"
//@ diag: 12:9 note "in instantiation of function template specialization 'f<int>' requested here"
template <class T> int f() { return T({1}); }

int i = int({3});
int j = f<int>();
