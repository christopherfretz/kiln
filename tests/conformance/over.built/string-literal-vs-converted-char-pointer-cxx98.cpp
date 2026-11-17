// C++98 variant of string-literal-vs-converted-char-pointer.cpp: the
// string literal to `char*` conversion is valid (deprecated), so the
// built-in `operator!=(char*, char*)` and `operator!=(const char*,
// const char*)` are indistinguishable for `s != "x"` and the call is
// ambiguous ([over.match.best]), as clang reports. Found running GCC's
// C++ testsuite.
//@ expect: compile-fail
//@ std: c++98
//@ skip(gcc): GCC 16.1 accepts it in C++98 too
//@ diag: 13:25 error "use of overloaded operator '!=' is ambiguous (with operand types 'S' and 'const char[2]')"

struct S { operator char *(); };
bool f(S &s) { return s != "x"; }
