// [lex.key] [lex.ccon]/3 `-fchar8_t` (clang `LangOptions::Char8`, GCC
// `-fchar8_t`): a later `-fchar8_t` restores the C++20 default after
// `-fno-char8_t`, and in C++17 enables `char8_t` as a keyword, `u8`
// literals of type `char8_t` and `__cpp_char8_t`. Without it C++17 has
// no `char8_t` keyword. spec/lex.md "-fno-char8_t", WP-808.
//@ expect: compile-ok
//@ args: -std=c++17 -fno-char8_t -fchar8_t
#ifndef __cpp_char8_t
#error "__cpp_char8_t is not defined"
#endif
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
static_assert(same<decltype(u8'a'), char8_t>);
static_assert(same<decltype(u8"ab"), const char8_t (&)[3]>);
static_assert(!__is_identifier(char8_t));
char8_t c = u8'z';
static_assert(sizeof(char8_t) == 1 && char8_t(-1) > 0);
