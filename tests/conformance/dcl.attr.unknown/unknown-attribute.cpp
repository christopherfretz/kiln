// [dcl.attr.grammar]/6: unknown attributes are ignored (with a warning)
//@ expect: compile-ok
//@ strict-diags
//@ diag: 8:3 warning "unknown attribute 'frobnicate' ignored"
//@ diag: 9:3 warning "unknown attribute 'thing' ignored"
//@ requires: clang<22
// clang 22 emits warning: unknown attribute 'vendor::thing' ignored; WP-355 residual
[[frobnicate]] int x;
[[vendor::thing]] int y;
