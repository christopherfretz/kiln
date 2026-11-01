// [lex.phases]/1.1 (C++23, P2295R6): UTF-8 is a supported source encoding;
// a UTF-8 character in a literal and an identifier keeps its code point.
// WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
constexpr char32_t c = U'é';
static_assert(c == 0xE9);
static_assert(sizeof(u8"é") == 3);
constexpr int größe = 3;
static_assert(größe == 3);
