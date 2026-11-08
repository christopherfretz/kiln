// [ACLE] Fixed-length SVE types in memory, arithmetic and conversions (ARM ACLE §3.7.3.3,
// clang AttrDocs "arm_sve_vector_bits"; the mangling is
// `9__SVE_VLSI<sizeless type>Lj<N>EE`). In memory a fixed-length type is
// `<M x T>`; a parameter or return is its scalable register type,
// converted with `llvm.vector.insert`/`extract` (a predicate through
// `<vscale x 2 x i8>`). spec/landed/codegen/WP-810.md (WP-810).
// Host-restricted (no pinned `--target`): on an x86_64 host clang has no
// AArch64 sysroot and leaves `+outline-atomics` out of the features.
//@ requires: clang>=22
//@ requires: target-aarch64
//@ expect: ir
//@ std: c++20
//@ args: -march=armv8-a+sve -msve-vector-bits=512
typedef __SVInt32_t svint32_t; typedef __SVUint8_t svuint8_t;
typedef __SVFloat32_t svfloat32_t; typedef __SVFloat64_t svfloat64_t;
typedef __SVBool_t svbool_t;
typedef svint32_t fi32 __attribute__((arm_sve_vector_bits(512)));
typedef svuint8_t fu8 __attribute__((arm_sve_vector_bits(512)));
typedef svfloat32_t ff32 __attribute__((arm_sve_vector_bits(512)));
typedef svfloat64_t ff64 __attribute__((arm_sve_vector_bits(512)));
typedef svbool_t fb __attribute__((arm_sve_vector_bits(512)));
typedef int gi32 __attribute__((vector_size(64)));
typedef char gc __attribute__((vector_size(64)));

struct S { fi32 v; int x; fb p; fi32 a[2]; };
fi32 g;
fi32 arr[2];
fb gp;
S gs;

fi32 add(fi32 a, fi32 b) { return a + b; }
svint32_t tosv(fi32 a) { return a; }
fi32 fromsv(svint32_t a) { return a; }
void m(fi32, fb, ff64) {}
int el(fi32 a) { return a[3]; }
fb pr(fb a, fb b) { return a & b; }
fu8 lax(fi32 a) { return a; }
gi32 gnu1(svint32_t a) { return a; }
svint32_t gnu2(gi32 a) { return a; }
svint32_t gnu3(gc a) { return a; }
fi32 gnu4(gi32 a) { return a; }
int mem(S &s, int i) { return s.v[i] + s.a[1][i]; }
void set(S &s, svint32_t v) { s.a[0] = v; s.v = s.a[0] * 2; s.p = s.p | s.p; }
ff32 fma(ff32 a, ff32 b) { return a * b + 1.0f; }
fi32 cmp(fi32 a, fi32 b) { return a < b; }
void globals(svint32_t v, svbool_t p) { g = v; arr[1] = g; gp = p; }
