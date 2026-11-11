// C23 6.7.10p15 (initialization): an array whose element type is
// compatible with wchar_t, char16_t or char32_t may be initialized by
// a wide string literal of that kind; in C these are integer typedefs
// (C23 7.21, 7.30), not distinct types as in C++ ([lex.string]).
// gcc.c-torture: execute/wchar_t-1.c, execute/widechar-2.c.
//@ std: c17
//@ expect: run
//@ stdout: 5 100 2 98 98 3 113
#include <stdio.h>
typedef __WCHAR_TYPE__ wchar_t;
typedef __CHAR16_TYPE__ char16_t;
typedef __CHAR32_TYPE__ char32_t;
wchar_t s[5] = L"abcd";
wchar_t s2[] = (L"q");
char16_t u[] = u"ab";
char32_t w[] = U"abc";
int main(void) {
  printf("%d %d %d %d %d %d %d\n", (int)(sizeof s / sizeof s[0]), (int)s[3],
         (int)(sizeof s2 / sizeof s2[0]), (int)u[1], (int)w[1],
         (int)(sizeof u / sizeof u[0]), (int)s2[0]);
  return 0;
}
