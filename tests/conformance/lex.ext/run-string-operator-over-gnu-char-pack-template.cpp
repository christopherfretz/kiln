// [lex.ext]/5: a string literal with a ud-suffix calls the literal
// operator template whose non-type template parameter the string can
// initialize, else `operator""X(str, len)`. The GNU extension
// `template<class C, C...> operator""X()` (a pack of the characters) is
// used only when no `(str, len)` operator exists: clang and GCC prefer
// the standard operator. kiln called the template. A C++20 class-type
// template parameter still wins over `(str, len)`, as the standard says
// (clang agrees; GCC 16.1 calls the `(str, len)` operator, hence the
// skip). Found running GCC's C++ testsuite.
//@ expect: run
//@ std: c++20
//@ args: -w
//@ skip(gcc): GCC 16.1 prefers operator""_n(const char*, size_t) over the class-type template
//@ stdout: 2 3 2 1

#include <cstddef>
#include <cstdio>

template <typename C, C... cs> int operator""_s() { return 1; }
int operator""_s(const char *, std::size_t) { return 2; }
int operator""_s(const char16_t *, std::size_t) { return 2; }

template <typename C, C... cs> int operator""_p() { return sizeof...(cs); }

struct Fixed {
    constexpr Fixed(const char (&)[4]) {}
};
template <Fixed f> int operator""_n() { return 1; }
int operator""_n(const char *, std::size_t) { return 2; }

int main() { std::printf("%d %d %d %d\n", "123"_s, "abc"_p, u"ab"_s, "abc"_n); }
