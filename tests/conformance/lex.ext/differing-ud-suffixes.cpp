// [lex.string.concat]/2 ([lex.ext]/8): concatenating string literals with
// different ud-suffixes is ill-formed. Found by the clang negative-test run
// (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, differing user-defined suffixes in concatenation are accepted
//@ expect: compile-fail
//@ diag: 10:23 error "differing user-defined suffixes ('_x' and '_y') in string literal concatenation"
const char *operator""_x(const char *, decltype(sizeof 0));
const char *operator""_y(const char *, decltype(sizeof 0));
const char *s = "a"_x "b"_y;
