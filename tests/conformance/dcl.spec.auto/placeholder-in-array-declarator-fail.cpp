// [dcl.type.auto.deduct]/2.1, [temp.deduct.type]/8: the bound of an array
// declarator must match; `decltype(auto)` admits no declarator operator
// ([dcl.type.auto.deduct]/5); an array of `auto` is deduced against the
// decayed initializer and fails ([temp.deduct.call]/2).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:8 error "variable 'p' with type 'auto (*)[1]' has incompatible initializer of type 'int (*)[2]'"
//@ diag: 12:20 error "cannot form array of 'decltype(auto)'"
//@ diag: 13:6 error "variable 'y' with type 'auto[2]' has incompatible initializer of type 'int[2]'"
int v[2];
auto (*p)[1] = &v;
decltype(auto) (*q)[1] = &v;
auto y[2] = v;
