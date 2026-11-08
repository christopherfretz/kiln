// [ACLE] Fixed-length SVE types have a size (ARM ACLE §3.7.3.3, clang
// AttrDocs "arm_sve_vector_bits"): a data vector is N/8 bytes aligned to
// 16, a predicate N/64 bytes aligned to 2; both may be members, array
// elements, globals and template arguments, and keep their size through
// cv-qualification and further typedefs. spec/types.md "Fixed-length
// SVE types" (WP-810).
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sve -msve-vector-bits=1024
#include <stddef.h>
#define N 1024
typedef __SVInt8_t svint8_t; typedef __SVUint16_t svuint16_t;
typedef __SVInt32_t svint32_t; typedef __SVUint64_t svuint64_t;
typedef __SVFloat16_t svfloat16_t; typedef __SVBfloat16_t svbfloat16_t;
typedef __SVFloat32_t svfloat32_t; typedef __SVFloat64_t svfloat64_t;
typedef __SVBool_t svbool_t;

typedef svint8_t fi8 __attribute__((arm_sve_vector_bits(N)));
typedef svuint16_t fu16 __attribute__((arm_sve_vector_bits(N)));
typedef svint32_t fi32 __attribute__((arm_sve_vector_bits(N)));
typedef svuint64_t fu64 __attribute__((arm_sve_vector_bits(N)));
typedef svfloat16_t ff16 __attribute__((arm_sve_vector_bits(N)));
typedef svbfloat16_t fbf16 __attribute__((arm_sve_vector_bits(N)));
typedef svfloat32_t ff32 __attribute__((arm_sve_vector_bits(N)));
typedef svfloat64_t ff64 __attribute__((arm_sve_vector_bits(N)));
typedef svbool_t fb __attribute__((arm_sve_vector_bits(N)));
typedef fi32 fi32_again;
typedef const volatile fi32 cvfi32;

static_assert(sizeof(fi8) == N / 8 && alignof(fi8) == 16);
static_assert(sizeof(fu16) == N / 8 && alignof(fu16) == 16);
static_assert(sizeof(fi32) == N / 8 && alignof(fi32) == 16);
static_assert(sizeof(fu64) == N / 8 && alignof(fu64) == 16);
static_assert(sizeof(ff16) == N / 8 && alignof(ff16) == 16);
static_assert(sizeof(fbf16) == N / 8 && alignof(fbf16) == 16);
static_assert(sizeof(ff32) == N / 8 && alignof(ff32) == 16);
static_assert(sizeof(ff64) == N / 8 && alignof(ff64) == 16);
static_assert(sizeof(fb) == N / 64 && alignof(fb) == 2);
static_assert(sizeof(fi32_again) == N / 8 && sizeof(cvfi32) == N / 8);

struct S { char c; fi32 v; short s; fb p; int x; fu64 w[2]; };
static_assert(offsetof(S, v) == 16);
static_assert(offsetof(S, s) == 16 + N / 8);
static_assert(offsetof(S, p) == 18 + N / 8);
static_assert(offsetof(S, x) == 20 + N / 8 + N / 64);
static_assert(offsetof(S, w) % 16 == 0);
static_assert(sizeof(S) == offsetof(S, w) + 2 * N / 8);
static_assert(alignof(S) == 16);

fi32 g;
fb gp;
ff64 arr[3];
static_assert(sizeof(arr) == 3 * N / 8);
fb parr[5];
static_assert(sizeof(parr) == 5 * N / 64);

template <class T> struct Box { T t; char c; };
static_assert(sizeof(Box<fi32>) == N / 8 + 16);
static_assert(sizeof(Box<fb>) == N / 64 + 2);

union U { fi32 v; int a[N / 32]; fb p; };
static_assert(sizeof(U) == N / 8);

int lanes(fi32 v) { return sizeof(v) / sizeof(v[0]); }
