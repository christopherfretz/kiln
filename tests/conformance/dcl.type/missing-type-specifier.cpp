// [dcl.type]/2: a declaration needs a type specifier (no implicit int)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:7 error "a type specifier is required for all declarations"
//@ diag: 7:8 error "a type specifier is required for all declarations"
const x = 1;
static y;
