// [dcl.attr.grammar] An attribute clang implements that changes
// semantics, the ABI or layout (class A) and kiln does not implement is
// an error, also in a system header (the lines after the line marker
// with flag 3, [cpp.line] GNU extension), where warnings are suppressed:
// kiln never accepts a construct whose meaning it would change
// (spec/sema.md §4.5). clang implements both. WP-805; WP-809
// implemented the `selectany` and `ms_struct` this test first used,
// WP-1745 its `weak_import`.
//@ skip(clang): kiln-only error for attributes kiln does not implement yet
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:20 error "'address_space' attribute is not supported by kiln yet"
//@ diag: 14:35 error "matrix types extension is disabled. Pass -fenable-matrix to enable it"
# 13 "tests/conformance/dcl.attr.unknown/class-a-error-in-system-header.cpp" 3
int __attribute__((address_space(1))) *as;
typedef float m2x2 __attribute__((matrix_type(2, 2)));
