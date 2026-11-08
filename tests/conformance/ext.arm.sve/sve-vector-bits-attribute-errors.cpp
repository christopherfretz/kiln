// [ACLE] `arm_sve_vector_bits(N)` (ARM ACLE §3.7.3.3, clang AttrDocs
// "arm_sve_vector_bits"): only on a typedef of an SVE data or predicate
// type (not svcount_t), with one integer constant argument equal to
// `-msve-vector-bits`; a dependent argument is not accepted (clang 19
// oracle wording). spec/types.md "Fixed-length SVE types" (WP-810).
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sve -msve-vector-bits=512
//@ strict-diags
//@ diag: 24:22 error "'arm_sve_vector_bits' attribute applied to non-SVE type 'int'"
//@ diag: 25:28 error "'arm_sve_vector_bits' attribute only applies to typedefs"
//@ diag: 26:35 error "'arm_sve_vector_bits' attribute only applies to typedefs"
//@ diag: 27:39 error "invalid SVE vector size '256', must match value set by '-msve-vector-bits' ('512')"
//@ diag: 29:31 error "'arm_sve_vector_bits' attribute applied to non-SVE type 'fi' (vector of 16 'int' values)"
//@ diag: 30:39 error "'arm_sve_vector_bits' attribute only applies to typedefs"
//@ diag: 31:32 error "'arm_sve_vector_bits' attribute applied to non-SVE type 'int *'"
//@ diag: 32:38 error "'arm_sve_vector_bits' attribute takes one argument"
//@ diag: 33:38 error "'arm_sve_vector_bits' attribute takes one argument"
//@ diag: 34:38 error "'arm_sve_vector_bits' attribute requires an integer constant"
//@ diag: 35:63 error "'arm_sve_vector_bits' attribute requires an integer constant"
//@ diag: 37:37 error "'arm_sve_vector_bits' attribute applied to non-SVE type 'svcount_t' (aka '__SVCount_t')"
//@ diag: 41:39 error "'arm_sve_vector_bits' attribute applied to non-SVE type 'svint32_t *' (aka '__SVInt32_t *')"
typedef __SVInt32_t svint32_t; typedef __SVUint32_t svuint32_t; typedef __SVBool_t svbool_t;
typedef __SVFloat64_t svfloat64_t; typedef __SVCount_t svcount_t;
int x __attribute__((arm_sve_vector_bits(512)));
svint32_t y __attribute__((arm_sve_vector_bits(512)));
void f(svint32_t z __attribute__((arm_sve_vector_bits(512))));
void g() { svint32_t w __attribute__((arm_sve_vector_bits(256))); }
typedef svint32_t fi __attribute__((arm_sve_vector_bits(512)));
typedef fi fi2 __attribute__((arm_sve_vector_bits(512)));
struct S { svint32_t m __attribute__((arm_sve_vector_bits(512))); };
typedef int *ip __attribute__((arm_sve_vector_bits(512)));
typedef svint32_t fi3 __attribute__((arm_sve_vector_bits(512, 2)));
typedef svint32_t fi4 __attribute__((arm_sve_vector_bits));
typedef svint32_t fi5 __attribute__((arm_sve_vector_bits(512.0)));
template<int N> struct T { typedef svint32_t t __attribute__((arm_sve_vector_bits(N))); };
typedef svbool_t fb __attribute__((arm_sve_vector_bits(512)));
typedef svcount_t fc __attribute__((arm_sve_vector_bits(512)));
typedef const svint32_t fcc __attribute__((arm_sve_vector_bits(512)));
fcc gcc = {};
static_assert(sizeof(fcc)==64);
typedef svint32_t *pfi __attribute__((arm_sve_vector_bits(512)));
