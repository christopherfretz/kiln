// [over.load]/2: functions cannot differ only in their return type
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:6 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 6:5 note "previous declaration is here"
int g();
long g();
