// [lex.ccon]/3 [lex.string]/6 [lex.key] `-fno-char8_t` (clang
// `LangOptions::Char8` off; GCC `-fno-char8_t`): `char8_t` is an ordinary
// identifier, a `u8` character literal is a `char` and a `u8` string
// literal a `const char[N]`, so overloads, template arguments and array
// initialization see `char`, and `__cpp_char8_t` is not defined.
// spec/lex.md "-fno-char8_t", WP-808.
//@ expect: run
//@ args: -fno-char8_t
//@ stdout: char char 3|xy|c
extern "C" int printf(const char *, ...);
#ifdef __cpp_char8_t
#error "__cpp_char8_t is defined"
#endif
int char8_t = 3;
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
static_assert(same<decltype(u8'a'), char>);
static_assert(same<decltype(u8"ab"), const char (&)[3]>);
static_assert(same<decltype(u8"ab"[0]), const char &>);
static_assert(__is_identifier(char8_t));
const char *which(const char *) { return "char"; }
const char *which(const unsigned char *) { return "unsigned char"; }
const char *which(const signed char *) { return "signed char"; }
template <class T> const char *tname() { return "other"; }
template <> const char *tname<char>() { return "char"; }
char arr[] = u8"xy";
const char *p = u8"c";
int main() {
  printf("%s %s %d|%s|%s\n", which(u8"x"), tname<decltype(u8'q')>(), char8_t, arr, p);
}
