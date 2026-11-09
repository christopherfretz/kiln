// C11 6.5.1.1 (_Generic in C++, a clang extension): under -pedantic the
// keyword draws '_Generic' is a C11 extension [-Wc11-extensions] and a
// type operand 'passing a type argument ... is a C2y extension'
// [-Wc2y-extensions]; a controlling expression with side effects is not
// evaluated [-Wunevaluated-expression]; qualified non-class association
// types can never be selected after lvalue conversion
// [-Wunreachable-code-generic-assoc].
//@ expect: compile-fail
//@ args: -pedantic
//@ strict-diags
//@ diag: 21:9 warning "'_Generic' is a C11 extension"
//@ diag: 21:19 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 22:9 warning "'_Generic' is a C11 extension"
//@ diag: 22:18 warning "passing a type argument as the first operand to '_Generic' is a C2y extension"
//@ diag: 22:31 error "type 'int' in generic association compatible with previously specified type 'int'"
//@ diag: 22:23 note "compatible type 'int' specified here"
//@ diag: 23:9 warning "'_Generic' is a C11 extension"
//@ diag: 23:27 warning "due to lvalue conversion of the controlling expression, association of type 'const int' will never be selected because it is qualified"
//@ diag: 23:44 warning "due to lvalue conversion of the controlling expression, association of type 'volatile int' will never be selected because it is qualified"
int x;
int y = _Generic(x++, int: 1);
int z = _Generic(int, int: 1, int: 2);
int w = _Generic(x, const int: 1, volatile int: 2, int: 3);
