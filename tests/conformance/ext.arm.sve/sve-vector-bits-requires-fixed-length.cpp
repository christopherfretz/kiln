// [ACLE] `arm_sve_vector_bits` needs a fixed vector length: without
// `-msve-vector-bits=<bits>` (or with `=scalable`) the attribute is an
// error (clang 19 oracle wording). spec/types.md "Fixed-length SVE types"
// (WP-810).
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sve
//@ strict-diags
//@ diag: 12:37 error "'arm_sve_vector_bits' is only supported when '-msve-vector-bits=<bits>' is specified with a value of 128, 256, 512, 1024 or 2048"
//@ diag: 13:36 error "'arm_sve_vector_bits' is only supported when '-msve-vector-bits=<bits>' is specified with a value of 128, 256, 512, 1024 or 2048"
typedef __SVInt32_t svint32_t;
typedef __SVBool_t svbool_t;
typedef svint32_t fi __attribute__((arm_sve_vector_bits(512)));
typedef svbool_t fb __attribute__((arm_sve_vector_bits(128)));
