// [expr.const]/10 (C++26, P3068R6), [lex.charset]: the what() string of
// an uncaught exception is program-provided text, so its note escapes it
// as clang escapes a static_assert message (clang `pushEscapedString`,
// spec/diag.md §6.2; WP-1259: kiln printed it raw, and lossily). ASCII
// whitespace prints as is (a newline continues the note on the next
// line, a tab stays a tab), any other control character as <U+XXXX>, a
// line or paragraph separator as <U+XXXX>, a byte that is not part of
// valid UTF-8 as <XX>; a format character (U+00AD) prints as is.
// GCC 16.1 differs: it prints every non-printable ASCII byte and every
// byte that is not valid UTF-8 as a C escape (\x0a, \x09, \x01, \xff)
// and keeps valid UTF-8, U+2028 included, so its note is always one
// line. Neither clang 19 nor clang 22 implements P3068; clang's
// rendering of a static_assert message is the precedent kiln follows.
//@ expect: compile-fail
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ strict-diags
//@ diag: 42:15 error "constexpr variable 'nl' must be initialized by a constant expression"
//@ diag: 41:38 note "unhandled exception: first line"
//@ diag: 43:15 error "constexpr variable 'tab' must be initialized by a constant expression"
//@ diag: 41:38 note "unhandled exception: a	b"
//@ diag: 44:15 error "constexpr variable 'ctl' must be initialized by a constant expression"
//@ diag: 41:38 note "unhandled exception: c<U+0001>d<U+007F>e<U+001B>[0m"
//@ diag: 45:15 error "constexpr variable 'bad' must be initialized by a constant expression"
//@ diag: 41:38 note "unhandled exception: f<FF>g<C3>(h<ED><A0><80>"
//@ diag: 46:15 error "constexpr variable 'sep' must be initialized by a constant expression"
//@ diag: 41:38 note "unhandled exception: i<U+2028>j<U+0085>k­l"
//@ diag: 48:1 error "consteval block is not a constant expression"
//@ diag: 48:13 note "unhandled exception: cb<U+0001><FF>"
namespace std {
struct exception {
  constexpr exception() noexcept {}
  constexpr virtual ~exception() {}
  constexpr virtual const char *what() const noexcept { return "std::exception"; }
};
} // namespace std
struct error : std::exception {
  const char *m;
  constexpr error(const char *m) : m(m) {}
  constexpr const char *what() const noexcept override { return m; }
};
constexpr int raise(const char *m) { throw error(m); }
constexpr int nl = raise("first line\nsecond line");
constexpr int tab = raise("a\tb");
constexpr int ctl = raise("c\x01" "d\x7f" "e\x1b[0m");
constexpr int bad = raise("f\xff" "g\xc3(h\xed\xa0\x80");
constexpr int sep = raise("i j\u0085k­l");
// A consteval block ([dcl.pre]/1, P2996R13) gets the same note.
consteval { throw error("cb\x01" "\xff"); }
