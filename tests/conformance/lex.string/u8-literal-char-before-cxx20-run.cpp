// [lex.string] (C++17 [lex.string]/8; P0482 for C++20): before C++20 a
// UTF-8 string literal is an array of `const char`, not `const char8_t`,
// matching `u8'x'`, which the lexer already types `char`. So it
// initializes a `std::string`, binds to `const char *`, and its
// user-defined literal operator takes `const char *`. abseil
// absl/strings/internal/utf8_test.cc (`std::pair<char32_t, std::string>`
// from `u8"£"`, WP-686). Verified against clang 19.
//@ expect: run
//@ std: c++17
//@ stdout: 2 3 4 1 6
#include <cstdio>
#include <string>
#include <utility>
template <class A, class B> struct same { static constexpr bool v = false; };
template <class A> struct same<A, A> { static constexpr bool v = true; };
static_assert(same<decltype(u8"a"), const char (&)[2]>::v, "");
static_assert(same<decltype(u8"£" "x"), const char (&)[4]>::v, "");
static_assert(u8"\xff"[0] == '\xff', "");
int operator""_n(const char *, std::size_t n) { return static_cast<int>(n); }
int main() {
  std::pair<char32_t, std::string> t[] = {{0x00A3, u8"£"}, {0x20AC, u8"€"}};
  const char *p = u8"\U00010000";
  std::printf("%d %d %d %d %d\n", int(t[0].second.size()), int(t[1].second.size()),
              int(std::string(p).size()), int(t[0].first == 0xA3), u8"abcdef"_n);
}
