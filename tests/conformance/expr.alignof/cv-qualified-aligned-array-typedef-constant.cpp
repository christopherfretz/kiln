// [expr.alignof]/3, [basic.type.qualifier]/3: cv-qualifiers on an array
// type are its elements' and do not change alignment, so a constant
// `alignof` of a cv-qualified array whose typedef carries GNU
// `__attribute__((aligned))` is the typedef's alignment, through a
// typedef of a qualified typedef, an array of the typedef (an array's
// alignment is its element's, also without cv: `T[2]`, and for a scalar
// or class typedef) and `__alignof__`. A plain or `alignas` array (no attributed typedef in the
// chain) keeps the element alignment: the walk adds nothing for it.
// Found running GCC's C++ testsuite.
//@ std: c++20
//@ expect: compile-ok
typedef double T[4][4] __attribute__((aligned(32)));
typedef const T CT;
typedef volatile CT CVT;
typedef CT A2[2];
typedef double P[4][4];
struct alignas(64) W { char c; };
typedef int I4[4];
typedef I4 I __attribute__((aligned(16)));
struct E { char c[64]; };
typedef E EA __attribute__((aligned(64)));
typedef W WA[3];
static_assert(alignof(T) == 32);
static_assert(alignof(const T) == 32);
static_assert(alignof(volatile T) == 32);
static_assert(alignof(const volatile T) == 32);
static_assert(alignof(CT) == 32);
static_assert(__alignof__(CVT) == 32);
static_assert(alignof(A2) == 32);
static_assert(alignof(const A2) == 32);
static_assert(alignof(const T[3]) == 32);
static_assert(alignof(T[2]) == 32);
static_assert(alignof(I[2]) == 16);
static_assert(alignof(const I[2][3]) == 16);
static_assert(alignof(EA[2]) == 64);
static_assert(alignof(const P) == alignof(double));
static_assert(alignof(const double[4][4]) == alignof(double));
static_assert(alignof(const WA) == 64);
static_assert(alignof(const W[2][2]) == 64);
constexpr int f() { return alignof(const T) + alignof(CT[2]); }
static_assert(f() == 64);
