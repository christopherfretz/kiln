// [dcl.attr.grammar] A class-A attribute (one clang rejects for every unit
// kiln compiles, spec/sema.md §4.5) is clang's error, also in a system
// header (the lines after the line marker with flag 3, [cpp.line] GNU
// extension), where warnings are suppressed. WP-805; the attributes this
// test first used (`selectany`, `ms_struct`, `weak_import`,
// `address_space`, `swift_context`) are implemented since WP-809,
// WP-1745 and WP-1803.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:35 error "matrix types extension is disabled. Pass -fenable-matrix to enable it"
//@ diag: 14:32 error "'riscv_rvv_vector_bits' attribute is not supported on targets missing 'zve32x'"
# 13 "tests/conformance/dcl.attr.unknown/class-a-error-in-system-header.cpp" 3
typedef float m2x2 __attribute__((matrix_type(2, 2)));
typedef int rvv __attribute__((riscv_rvv_vector_bits(128)));
