// [expr.static.cast]: no static_cast from a pointer to an integer
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:10 error "static_cast from 'int *' to 'long' is not allowed"
int *p;
long l = static_cast<long>(p);
