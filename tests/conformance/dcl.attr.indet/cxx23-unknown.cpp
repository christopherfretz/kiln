// [dcl.attr.indet] is C++26 (P2795R5); before it `indeterminate` is not a
// standard attribute, so [dcl.attr.grammar]/6 lets it be ignored: kiln
// warns as clang does for any unknown attribute, writes no erroneous value
// (codegen is unchanged before C++26), and `__has_cpp_attribute` is 0.
//@ expect: compile-ok
//@ strict-diags
//@ std: c++23
//@ requires: clang>=23
//@ diag: 10:20 warning "unknown attribute 'indeterminate' ignored"
void f() { int x [[indeterminate]]; (void)x; }
static_assert(__has_cpp_attribute(indeterminate) == 0);
