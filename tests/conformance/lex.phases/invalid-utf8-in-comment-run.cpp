// [lex.phases]/1, [lex.comment]: invalid UTF-8 inside a comment is accepted (clang warns only
// under -Winvalid-utf8, off by default); OpenCV's half.cpp has Latin-1 in a comment. Café.
//@ expect: run
//@ stdout: 7
#include <cstdio>
/* block comment ÿþ bytes */
int main() { std::printf("%d\n", 7); }
