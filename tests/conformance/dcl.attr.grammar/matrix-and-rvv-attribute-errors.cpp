// [dcl.attr.grammar]/6 (implementation-defined attributes): clang's
// `matrix_type` needs `-fenable-matrix`, which kiln does not offer, and
// `riscv_rvv_vector_bits` needs the RISC-V vector extension, which no
// target kiln compiles for has; both are clang's errors.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:35 error "matrix types extension is disabled. Pass -fenable-matrix to enable it"
//@ diag: 11:32 error "'riscv_rvv_vector_bits' attribute is not supported on targets missing 'zve32x'"

typedef float m4x4 __attribute__((matrix_type(4, 4)));
typedef int rvv __attribute__((riscv_rvv_vector_bits(128)));
