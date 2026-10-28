// [stmt.break]/1: break needs an enclosing loop or switch
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:12 error "'break' statement not in loop or switch statement"
void f() { break; }
