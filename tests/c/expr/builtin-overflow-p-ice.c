// GNU __builtin_*_overflow_p with constant operands is an integer
// constant expression when its third argument has no side effects and
// calls no function (a bit-field, a non-constant variable): only its type
// is used (builtin-overflow-p-errors.c has the others). So it works in
// _Static_assert, an array bound, an enumerator and a bit-field width,
// which GCC 16.1 accepts alike. WP-1549.
//@ std: c17
//@ expect: compile-ok
//@ oracle: gcc
//@ skip(clang): clang 19/22 do not implement __builtin_*_overflow_p
//@ strict-diags
struct B { int s3 : 3; unsigned u5 : 5; } bf;
int x = 3;
_Static_assert(__builtin_add_overflow_p(1, 2, (int)0) == 0, "");
_Static_assert(__builtin_add_overflow_p(__INT_MAX__, 1, (int)0) == 1, "");
_Static_assert(__builtin_add_overflow_p(200, 100, (unsigned char)0) == 1, "");
_Static_assert(__builtin_sub_overflow_p(0u, 1u, 0) == 0, "");
_Static_assert(__builtin_mul_overflow_p(-1, 1, 0u) == 1, "");
_Static_assert(__builtin_add_overflow_p(3, 1, bf.s3) == 1, "");
_Static_assert(__builtin_add_overflow_p(30, 1, bf.u5) == 0, "");
_Static_assert(__builtin_add_overflow_p(31, 1, bf.u5) == 1, "");
_Static_assert(__builtin_add_overflow_p(1, 2, x) == 0, "");
_Static_assert(__builtin_mul_overflow_p(1 << 16, 1 << 16, x + 1) == 1, "");
_Static_assert(__builtin_mul_overflow_p(1LL << 62, 2, 0ULL) == 0, "");
_Static_assert(__builtin_mul_overflow_p(1LL << 62, 4, 0ULL) == 1, "");
_Static_assert(_Generic(__builtin_add_overflow_p(1, 2, 0), _Bool: 1, default: 0), "");
int arr[__builtin_mul_overflow_p(1 << 20, 1 << 20, 0LL) ? -1 : 1];
enum { K = __builtin_add_overflow_p(1, 2, 0) + 1 };
_Static_assert(K == 1, "");
struct S { int w : __builtin_add_overflow_p(__INT_MAX__, 1, 0) + 1; };
