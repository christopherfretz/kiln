// [expr.alignof]/3: the typedef's alignment is kept under cv-qualifiers,
// so asserting the element alignment of a cv-qualified aligned array
// typedef fails (rejected counterpart of
// cv-qualified-aligned-array-typedef-constant.cpp).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:15 error "static assertion failed due to requirement 'alignof(const double[4][4]) == alignof(double)'"
//@ diag: 22:32 note "expression evaluates to '32 == 8'"
//@ diag: 23:15 error "static assertion failed due to requirement 'alignof(volatile double[4][4]) == 8'"
//@ diag: 23:35 note "expression evaluates to '32 == 8'"
//@ diag: 24:15 error "static assertion failed due to requirement '__alignof(const double[4][4]) < 32'"
//@ diag: 24:31 note "expression evaluates to '32 < 32'"
//@ diag: 25:15 error "static assertion failed due to requirement 'alignof(const double[2][4][4]) == 8'"
//@ diag: 25:33 note "expression evaluates to '32 == 8'"
//@ diag: 28:15 error "static assertion failed due to requirement 'alignof(int[2][4]) == alignof(int)'"
//@ diag: 28:29 note "expression evaluates to '16 == 4'"
//@ diag: 30:15 error "static assertion failed due to requirement 'f()'"
typedef double T[4][4] __attribute__((aligned(32)));
typedef const T CT;
typedef CT A2[2];
static_assert(alignof(const T) == alignof(double));
static_assert(alignof(volatile T) == 8);
static_assert(__alignof__(CT) < 32);
static_assert(alignof(const A2) == 8);
typedef int I4[4];
typedef I4 I __attribute__((aligned(16)));
static_assert(alignof(I[2]) == alignof(int));
constexpr bool f() { return alignof(const T[2]) == 8; }
static_assert(f());
