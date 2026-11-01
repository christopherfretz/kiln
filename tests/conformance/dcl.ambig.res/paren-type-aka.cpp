//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:7 error "cannot initialize a variable of type 'long *' with an lvalue of type 'int (*)'"
//@ diag: 15:7 error "cannot initialize a variable of type 'long *' with an lvalue of type 'I (*)' (aka 'int *')"
//@ diag: 16:7 error "cannot initialize a variable of type 'long *' with an lvalue of type 'int[2]'"
//@ diag: 18:7 error "cannot initialize a variable of type 'long *' with an lvalue of type 'int (*)(int)'"
// Grouping parentheses in a declarator are ParenType sugar: diagnostics
// print them as written (`int (*)`), and clang's desugarForDiagnostic
// looks through them without an `aka` of its own. WP-191.
typedef int I;
int (*p1);
I (*p2);
int (&r1)[2] = *(int(*)[2])0;
long *l1 = p1;
long *l2 = p2;
long *l3 = r1;
int (*(fp))(int);
long *l4 = fp;
