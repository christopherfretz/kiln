// [expr.pre.incr]/1, [conv.integral]/3: with `-fsigned-char` plain `char`
// is signed and `++` past CHAR_MAX wraps to CHAR_MIN through the promoted
// `int`; with `-funsigned-char` (the sibling test) it wraps 255 to 0.
// libc++ 22 stable_sort.pass.cpp `static_assert(test<char>())` (WP-980).
//@ expect: compile-ok
//@ std: c++26
//@ args: -fsigned-char
constexpr int pre_inc() { char x = 127; ++x; return x; }
static_assert(pre_inc() == -128);
constexpr int post_inc() { char x = 127; char old = x++; return old * 1000 + x; }
static_assert(post_inc() == 127 * 1000 - 128);
constexpr int pre_dec() { char x = -128; --x; return x; }
static_assert(pre_dec() == 127);
constexpr int post_dec() { char x = -128; x--; return x; }
static_assert(post_dec() == 127);
