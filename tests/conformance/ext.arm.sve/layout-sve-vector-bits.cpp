// [ACLE] The record layout of fixed-length SVE members (ARM ACLE §3.7.3.3):
// a data vector is N/8 bytes aligned to 16, a predicate N/64 bytes aligned
// to 2. Compared with clang's record layout dump. spec/types.md
// "Fixed-length SVE types" (WP-810).
//@ expect: layout
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sve -msve-vector-bits=256
typedef __SVInt32_t svint32_t; typedef __SVFloat64_t svfloat64_t;
typedef __SVBool_t svbool_t;
typedef svint32_t fi32 __attribute__((arm_sve_vector_bits(256)));
typedef svfloat64_t ff64 __attribute__((arm_sve_vector_bits(256)));
typedef svbool_t fb __attribute__((arm_sve_vector_bits(256)));
struct A { char c; fi32 v; short s; fb p; int x; };
struct B : A { fb q[3]; ff64 d[2]; };
struct C { fb p; char c; };
A a; B b; C c;
