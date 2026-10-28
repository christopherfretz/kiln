// [dcl.pre]/12: a failed static_assert shows its message
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:15 error "static assertion failed due to requirement 'sizeof(int) == 2': need 16-bit int"
//@ diag: 8:27 note "expression evaluates to '4 == 2'"
//@ diag: 9:15 error "static assertion failed due to requirement '1 + 1 == 3'"
//@ diag: 9:21 note "expression evaluates to '2 == 3'"
static_assert(sizeof(int) == 2, "need 16-bit int");
static_assert(1 + 1 == 3);
