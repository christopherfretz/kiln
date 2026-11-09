// [lex.string.concat]/2: concatenating string literals with different
// ud-suffixes is ill-formed (reported at each differing piece); one suffix,
// alone or with plain pieces, stays valid. Found by the clang negative-test
// run (WP-912), fixed by WP-1017.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:23 error "differing user-defined suffixes ('_x' and '_y') in string literal concatenation"
//@ diag: 13:27 error "differing user-defined suffixes ('_x' and '_y') in string literal concatenation"
const char *operator""_x(const char *, decltype(sizeof 0));
const char *operator""_y(const char *, decltype(sizeof 0));
const char *s = "a"_x "b"_y;
const char *t = "a" "b"_x "c"_y "d"_x;
const char *ok1 = "a"_x "b"_x;
const char *ok2 = "a" "b"_x "c";
