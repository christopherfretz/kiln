// [ACLE] Aggregates of fixed-length SVE types by value, clang 19's
// lowering (AAPCS64 before pure scalable types): a homogeneous aggregate of
// up to four 128-bit vectors, SVE or NEON, is `[N x <4 x i32>]`, a small
// one is an integer, a larger one is indirect. Clang 20+ passes a pure
// scalable type in SVE registers, which kiln rejects (crates/kiln_driver/tests/codegen.rs
// `sve_pure_scalable_by_value`). spec/landed/codegen/WP-810.md (WP-810).
// Host-restricted (no pinned `--target`): on an x86_64 host clang has no
// AArch64 sysroot and leaves `+outline-atomics` out of the features.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ std: c++20
//@ args: -march=armv8-a+sve -msve-vector-bits=128
typedef __SVInt32_t svint32_t; typedef __SVBool_t svbool_t;
typedef svint32_t fi32 __attribute__((arm_sve_vector_bits(128)));
typedef svbool_t fb __attribute__((arm_sve_vector_bits(128)));
typedef __attribute__((neon_vector_type(4))) int n4;
struct A { fi32 v; };
struct B { fb p; };
struct C { fi32 v; n4 n; };
struct D { fi32 v[2]; fb p; };
void a(A) {}
void b(B) {}
void c(C) {}
void d(D) {}
A ra() { return {}; }
int call(A x, D y) { a(x); d(y); return ra().v[0]; }
