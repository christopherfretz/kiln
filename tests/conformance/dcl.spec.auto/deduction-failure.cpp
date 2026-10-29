// Placeholder deduction failures for auto with pointer and reference
// declarators ([dcl.type.auto.deduct]/4, [temp.deduct.call]).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:7 error "variable 'bad1' with type 'auto *' has incompatible initializer of type 'int'"
//@ diag: 13:8 error "variable 'bad2' with type 'auto **' has incompatible initializer of type 'int *'"
//@ diag: 14:7 error "variable 'bad3' with type 'auto *' has incompatible initializer of type 'int'"
//@ diag: 15:26 error "cannot deduce return type 'auto *' from returned value of type 'int'"
//@ diag: 16:28 error "cannot deduce return type 'auto **' from returned value of type 'int *'"
int i;
int *pi = &i;
auto *bad1 = 1;
auto **bad2 = &i;
auto *bad3 = i;
auto *ret_bad() { return 1; }
auto **ret_bad2() { return pi; }
