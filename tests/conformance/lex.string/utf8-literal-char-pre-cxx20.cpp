// [lex.string]: before C++20 a UTF-8 string literal is an array of
// `const char` (C++17 [lex.string]/8); from C++20 of `const char8_t`
// (P0482R6, [lex.string]/6, utf8-literal-char-cxx20.cpp). A user-defined
// string literal `u8"..."_s` then calls the `const char *` literal operator
// ([lex.ext]/5), and `__cpp_char8_t` is not defined.
//@ expect: run
//@ std: c++17
//@ strict-diags
template <class T, class U> struct same { static const bool value = false; };
template <class T> struct same<T, T> { static const bool value = true; };
static_assert(same<decltype(u8"ab"), const char (&)[3]>::value, "");
static_assert(same<decltype(u8'a'), char>::value, "");
static_assert(same<decltype(u8"a" "b"), const char (&)[3]>::value, "");
#ifdef __cpp_char8_t
#error "no char8_t before C++20"
#endif
struct S { unsigned long n; };
S operator""_s(const char *, decltype(sizeof 0) n) { return S{n}; }
const char *p = u8"é";
int main() {
  S s = u8"éx"_s;
  return (s.n == 3 && p[0] == '\xC3' && p[2] == 0) ? 0 : 1;
}
