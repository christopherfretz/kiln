// [dcl.attr.grammar]/6: unknown attributes are ignored (with a warning)
//@ expect: compile-ok
//@ strict-diags
//@ diag: 6:3 warning "unknown attribute 'frobnicate' ignored"
//@ diag: 7:3 warning "unknown attribute 'thing' ignored"
[[frobnicate]] int x;
[[vendor::thing]] int y;
