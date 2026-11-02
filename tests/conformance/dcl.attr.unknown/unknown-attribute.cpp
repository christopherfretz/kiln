// [dcl.attr.grammar]/6: unknown attributes are ignored (with a warning)
//@ expect: compile-ok
//@ strict-diags
//@ diag: 8:3 warning "unknown attribute 'frobnicate' ignored"
//@ diag: 9:3 warning "unknown attribute '"
// clang 19 and 22 word these differently: the substrings above are common
// to both, and kiln follows its -fclang-compat level (WP-377).
[[frobnicate]] int x;
[[vendor::thing]] int y;
