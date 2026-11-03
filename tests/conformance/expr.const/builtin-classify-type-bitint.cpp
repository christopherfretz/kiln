// [expr.const]/5, [basic.fundamental]/1: `__builtin_classify_type` of a
// `_BitInt(N)` (a C23 type clang accepts in C++) is GCC's bit-int class
// 18, signed or unsigned, whatever the width, also through a reference,
// a cv-qualifier, a typedef and a template argument. A `_BitInt` narrower
// than `int` meeting an `int` converts to `int` ([conv.rank]: class 1), a
// comparison is `bool` (4) and a pointer 5 (clang
// `EvaluateBuiltinClassifyType`, the same in clang 19 and 22). The other
// classes: builtin-classify-type.cpp (WP-419).
//@ expect: compile-ok
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
_BitInt(7) s7;
unsigned _BitInt(1) u1;
const _BitInt(128) s128 = 0;
_BitInt(8388608) *big;
typedef unsigned _BitInt(33) U33;
U33 u33;
extern _BitInt(9) &ref;
static_assert(__builtin_classify_type(s7) == 18);
static_assert(__builtin_classify_type(u1) == 18);
static_assert(__builtin_classify_type(s128) == 18);
static_assert(__builtin_classify_type(*big) == 18);
static_assert(__builtin_classify_type(u33) == 18);
static_assert(__builtin_classify_type(ref) == 18);
static_assert(__builtin_classify_type(s128 + 1) == 18);
static_assert(__builtin_classify_type(s7 + 1) == 1);
static_assert(__builtin_classify_type(s7 < s7) == 4);
static_assert(__builtin_classify_type(big) == 5);
static_assert(__builtin_classify_type((_BitInt(64))3) == 18);
template <class T> constexpr int cls(T v) { return __builtin_classify_type(v); }
static_assert(cls(U33()) == 18);
static_assert(cls<_BitInt(17)>(1) == 18);
static_assert(cls(true) == 4);
