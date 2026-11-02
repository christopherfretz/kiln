// [module.private.frag]/1: a private module fragment shall appear only in
// a primary module interface unit.
//@ expect: compile-fail
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
//@ diag: 8:9 error "private module fragment declaration with no preceding module declaration"
module :private;
int x;
