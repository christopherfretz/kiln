// [expr.pre.incr]/1, [expr.post.incr]/1: `++x` is `x += 1`, computed in
// the promoted type `int` and converted back ([conv.integral]/3: modulo
// 2^N since C++20), so stepping a `signed char` or `short` past its range
// is not overflow and is a constant expression (clang: the operator's
// `canOverflow()` is false for a type narrower than `int`). kiln's Oven
// steps in the narrow type and reports "value 128 is outside the range of
// representable values". On x86_64 plain `char` is signed, so libc++ 22's
// stable_sort.pass.cpp `static_assert(test<char>())` fails there
// (x86_64 sweep); AArch64's unsigned `char` hides it.
//@ expect: compile-ok
//@ std: c++26
//@ xfail(kiln): WP-961 follow-up (WP-981), constexpr ++/-- on a signed type narrower than int overflows in the narrow type instead of wrapping (seen in libc++ 22 stable_sort.pass.cpp on x86_64)
constexpr int pre_inc() { signed char x = 127; ++x; return x; }
static_assert(pre_inc() == -128);
constexpr int post_inc() { signed char x = 127; x++; return x; }
static_assert(post_inc() == -128);
constexpr int pre_dec() { short x = -32768; --x; return x; }
static_assert(pre_dec() == 32767);
constexpr int compound() { signed char x = 127; x += 1; return x; }
static_assert(compound() == -128);
