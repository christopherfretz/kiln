// [dcl.type.auto.deduct]/4: direct-list-initialization needs exactly one
// element.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:11 error "initializer for variable 'a' with type 'auto' contains multiple expressions"
//@ diag: 8:7 error "initializer for variable 'b' with type 'auto' is empty"
auto a{1, 2};
auto b{};
