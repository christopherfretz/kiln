// [basic.fundamental]/3, [conv.integral]/3, [expr.shift] (C++20, P1236R1 /
// P0907R4): signed integers are two's complement; conversion is modular
// and shifts of negative values are defined. WP-646 feature probe.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
static_assert(-1 >> 1 == -1 && -8 >> 2 == -2);
static_assert(-1 << 1 == -2 && (-3 << 3) == -24);
static_assert(static_cast<int>(0xFFFFFFFFu) == -1);
static_assert(static_cast<signed char>(200) == -56);
static_assert(static_cast<unsigned>(-1) == 0xFFFFFFFFu);
static_assert((-2147483647 - 1) == static_cast<int>(1u << 31));
static_assert(~0 == -1 && (-1 & 0xFF) == 255);
