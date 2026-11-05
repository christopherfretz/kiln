// [lex.icon], [lex.fcon] (C++14, N3472 binary literals; N3781 digit
// separators). WP-646 feature probe.
//@ expect: compile-ok
//@ std: c++14
//@ strict-diags
static_assert(0b101 == 5 && 0B1111'0000 == 240, "binary");
static_assert(0b1ull << 63 == 0x8000'0000'0000'0000ull, "binary suffix");
static_assert(1'000'000 == 1000000 && 1'2'3 == 123, "decimal");
static_assert(07'7 == 63 && 0xFF'FF == 65535, "octal and hex");
static_assert(1'0.2'5 == 10.25 && 1'2e1'0 > 0, "floating");
static_assert(sizeof(0b0) == sizeof(int) && sizeof(0b1L) == sizeof(long), "");
