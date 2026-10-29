// [dcl.type.auto.deduct]/2.2: a braced-init-list cannot be returned from a
// function with a deduced return type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:19 error "cannot deduce return type from initializer list"
//@ diag: 8:19 error "cannot deduce return type from initializer list"
auto f() { return {1}; }
auto g() { return {}; }
