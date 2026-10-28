// [expr.call]/7: each argument initializes its parameter
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:9 error "no matching function for call to 'g'"
//@ diag: 6:5 note "candidate function not viable: no known conversion from 'double' to 'int *' for 1st argument"
int g(int *);
int y = g(1.0);
