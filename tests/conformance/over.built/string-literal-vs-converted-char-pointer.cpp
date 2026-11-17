// [over.built]/16 equality candidates for `s != "x"` where `s` converts
// to `char*`: `operator!=(char*, char*)` needs the string literal to
// `char*` conversion (ill-formed since C++11, [diff.cpp03.lex]; clang
// accepts it as an extension), and `operator!=(const char*, const char*)`
// adds a qualification conversion after the user-defined conversion, so
// by [over.match.best] neither is better. clang (C++11 on) first prefers
// the candidate that needs no ill-formed string conversion for any
// argument (`isBetterOverloadCandidate`), GCC agrees; kiln reported the
// call ambiguous. The same holds for the other built-in operators and
// for user functions; in C++98 the conversion is valid and deprecated,
// and clang reports these ambiguous (c++98 variant:
// string-literal-vs-converted-char-pointer-cxx98.cpp). Found running
// GCC's C++ testsuite.
//@ expect: run
//@ std: c++17
//@ args: -w
//@ stdout: 1 0 1 2 2 2 2

#include <cstdio>

static char buf[] = "x";
struct S {
    operator char *() { return buf; }
};
// Converts to both: the `char*` target wins for `char*` operands only.
struct B {
    operator char *() { return buf; }
    operator const char *() { return "y"; }
};

int q(char *, int) { return 1; }
int q(const char *, long) { return 2; }
int g(char *) { return 1; }
int g(const char *) { return 2; }

int main() {
    S s;
    B b;
    bool ne = s != "x";
    bool eq = s == "x";
    bool lt = s < "x";
    const char *c = true ? s : "x";
    std::printf("%d %d %d %d %d %d %d\n", ne, eq, lt || !lt, q("x", 0), g("x"),
                c == buf ? 2 : 0, b != "x" ? 2 : 0);
}
