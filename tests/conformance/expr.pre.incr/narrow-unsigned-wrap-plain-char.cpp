// [expr.pre.incr]/1, [conv.integral]/3: with `-funsigned-char` plain
// `char` is unsigned, so `++` past 255 wraps to 0 and `--` below 0 to 255
// (WP-980; the `-fsigned-char` sibling wraps at 127).
//@ expect: compile-ok
//@ std: c++26
//@ args: -funsigned-char
constexpr int pre_inc() { char x = 127; ++x; return x; }
static_assert(pre_inc() == 128);
constexpr int wrap() { char x = (char)255; x++; return x; }
static_assert(wrap() == 0);
constexpr int pre_dec() { char x = 0; --x; return x; }
static_assert(pre_dec() == 255);
