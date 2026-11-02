// Constant evaluation of ext_vector_type element access (clang ext;
// [expr.const]): clang 19 folds an access on a prvalue vector (a compound
// literal, an operator result) for one lane and for a swizzle, and
// rejects one on a vector object without a note.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:15 error "static assertion expression is not an integral constant expression"
//@ diag: 29:15 error "static assertion expression is not an integral constant expression"
//@ requires: clang<22
// clang 22 expected compile-fail, but compilation succeeded; WP-355 residual
typedef float f4 __attribute__((ext_vector_type(4)));
typedef int i4 __attribute__((ext_vector_type(4)));
typedef int i3 __attribute__((ext_vector_type(3)));
constexpr f4 v = {1, 2, 3, 4};
static_assert((v + v).y == 4);
static_assert(((f4){1, 2, 3, 4}).s3 == 4);
static_assert(((f4){1, 2, 3, 4}).a == 4);
constexpr float h(float x) { return ((f4){x, 2, 3, 4}).x; }
static_assert(h(5) == 5);
constexpr int k() { return ((i4){1, 2, 3, 4}).hi.y + ((i4){1, 2, 3, 4}).odd.x; }
static_assert(k() == 6);
static_assert(__builtin_bit_cast(long long, ((i4){1, 2, 3, 4}).zw) == 0x400000003);
static_assert(__builtin_bit_cast(long long, ((i3){1, 2, 3}).even) == 0x300000001);
constexpr i4 sw = ((i4){1, 2, 3, 4}).wzyx;
static_assert(__builtin_bit_cast(__int128, sw) == __builtin_bit_cast(__int128, (i4){4, 3, 2, 1}));
// An element of a vector object is not folded by clang 19 (no note).
static_assert(v.y == 2);
constexpr int m() { i4 a = {1, 2, 3, 4}; return a.x; }
static_assert(m() == 1);
