// [dcl.fct.def.default]/1, [dcl.fct.def.delete]/1: only functions are defaulted or deleted
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:9 error "only special member functions and comparison operators may be defaulted"
//@ diag: 7:9 error "only functions can have deleted definitions"
int x = default;
int z = delete;
