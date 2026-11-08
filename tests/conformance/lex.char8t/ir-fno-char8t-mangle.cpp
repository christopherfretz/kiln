// [lex.ccon]/3 [lex.string]/6 Under `-fno-char8_t` a `u8` literal is a
// `char` (`c`, not `Du`, in mangled names), a `u8` string literal's
// object is a plain `[N x i8]` constant, and in `#if` a `u8` character
// literal is signed as `char` is on x86-64 (clang `PPExpressions`
// `Char8`). spec/lex.md "-fno-char8_t", WP-808.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -fno-char8_t
template <class T> void take(T) {}
template void take(decltype(u8'a'));
template void take(decltype(+u8"ab"));
void m(decltype(u8'x')) {}
const char *s = u8"text";
#if u8'\xff' < 0
int signed_u8 = 1;
#else
int signed_u8 = 0;
#endif
