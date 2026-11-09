// [expr.pre.incr]/1, [expr.post.incr]/1: `++x` is `x += 1`, computed in
// the promoted type `int` ([expr.arith.conv]) and converted back
// ([conv.integral]/3: modulo 2^N since C++20), so stepping a `signed
// char` or `short` past its range is not overflow and is a constant
// expression (clang: the operator's `canOverflow()` is false for a type
// narrower than `int`). The postfix forms yield the old value. Seen in
// libc++ 22 stable_sort.pass.cpp `static_assert(test<char>())` on x86_64,
// where plain `char` is signed (WP-980).
//@ expect: compile-ok
//@ std: c++26
constexpr int pre_inc() { signed char x = 127; ++x; return x; }
static_assert(pre_inc() == -128);
constexpr int post_inc() { signed char x = 127; signed char old = x++; return old * 1000 + x; }
static_assert(post_inc() == 127 * 1000 - 128);
constexpr int pre_dec() { short x = -32768; --x; return x; }
static_assert(pre_dec() == 32767);
constexpr int post_dec() { signed char x = -128; x--; return x; }
static_assert(post_dec() == 127);
constexpr int pre_inc_short() { short x = 32767; return ++x; }
static_assert(pre_inc_short() == -32768);
constexpr int compound() { signed char x = 127; x += 1; return x; }
static_assert(compound() == -128);
// Unsigned narrow types wrap as before.
constexpr int uchar() { unsigned char x = 255; ++x; unsigned short y = 0; --y; return x + y; }
static_assert(uchar() == 65535);
// A loop stepping through the whole range ends where it began.
constexpr bool full_cycle() {
  signed char c = 0;
  for (int i = 0; i < 256; ++i) ++c;
  for (int i = 0; i < 256; ++i) c--;
  return c == 0;
}
static_assert(full_cycle());
// Steps inside the range are unchanged.
constexpr int in_range() { short s = 10; s++; ++s; --s; return s; }
static_assert(in_range() == 11);
