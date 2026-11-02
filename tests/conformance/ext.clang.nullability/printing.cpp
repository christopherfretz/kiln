// A nullability qualifier is type sugar and prints as clang prints it
// (spec/sema.md §4.1.1).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:5 error "cannot initialize a variable of type 'int' with an lvalue of type 'int * _Nonnull'"
//@ diag: 11:5 error "cannot initialize a variable of type 'int' with an lvalue of type 'int * _Nullable * _Null_unspecified'"
//@ diag: 14:5 error "cannot initialize a variable of type 'int' with an lvalue of type 'NR' (aka 'int *')"
int* _Nonnull p1;
int x1 = p1;
int* _Nullable* _Null_unspecified p2;
int x2 = p2;
typedef int* _Nullable_result NR;
NR p3;
int x3 = p3;
