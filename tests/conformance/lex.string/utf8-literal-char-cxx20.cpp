// [lex.string]/6 (P0482R6): from C++20 a UTF-8 string literal is an array
// of `const char8_t`, and `u8"..."_s` calls the `const char8_t *` literal
// operator ([lex.ext]/5); before C++20 see utf8-literal-char-pre-cxx20.cpp.
//@ expect: run
//@ std: c++20
//@ strict-diags
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
static_assert(same<decltype(u8"ab"), const char8_t (&)[3]>);
static_assert(same<decltype(u8'a'), char8_t>);
struct S { int which; unsigned long n; };
S operator""_s(const char *, decltype(sizeof 0) n) { return S{1, n}; }
S operator""_s(const char8_t *, decltype(sizeof 0) n) { return S{8, n}; }
int main() {
  S a = u8"éx"_s;
  S b = "x"_s;
  return (a.which == 8 && a.n == 3 && b.which == 1) ? 0 : 1;
}
