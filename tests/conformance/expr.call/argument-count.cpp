// [expr.call]/7: the number of arguments must match
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:9 error "no matching function for call to 'f'"
//@ diag: 8:5 note "candidate function not viable: requires 2 arguments, but 1 was provided"
//@ diag: 10:9 error "no matching function for call to 'f'"
//@ diag: 8:5 note "candidate function not viable: requires 2 arguments, but 3 were provided"
int f(int, int);
int a = f(1);
int b = f(1, 2, 3);
