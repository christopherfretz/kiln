// Constant evaluation of ext_vector_type element access (clang ext;
// [expr.const]): clang 22 also folds a one-lane access of a vector object
// (clang 19 rejects it without a note).
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
// clang 22 spelling of element-access-constexpr-diagnostics.cpp (WP-394).
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
// A lane of a vector object is folded by clang 22.
static_assert(v.y == 2);
constexpr int m() { i4 a = {1, 2, 3, 4}; return a.x; }
static_assert(m() == 1);
