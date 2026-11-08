// [lex.key] C++17 has no `char8_t` keyword (P0482R6 added it in C++20;
// clang's `KEYCHAR8`): it is an ordinary identifier, `u8'a'` is a `char`
// ([lex.ccon]/3 in C++17), and `__cpp_char8_t` is not defined.
// spec/lex.md "-fno-char8_t", WP-808.
//@ expect: compile-ok
//@ args: -std=c++17
#ifdef __cpp_char8_t
#error "__cpp_char8_t is defined"
#endif
int char8_t;
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
static_assert(same<decltype(u8'a'), char>);
static_assert(same<decltype(u8"ab"), const char (&)[3]>);
