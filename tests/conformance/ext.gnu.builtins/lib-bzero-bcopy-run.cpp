//@ expect: run
//@ args: -std=gnu++26
//@ stdout: ababcdefij cdefghghij abc....hij 0123456789
//@ stdout: abcd 4
// BSD bzero / bcopy and GNU mempcpy (clang EmitBuiltinExpr, WP-1073): in
// the GNU dialects they are builtins lowered to memset with 0, memmove
// (source first, so overlapping copies in either direction are correct)
// and memcpy returning the end of the destination.
#include <cstdio>
#include <cstring>
#include <strings.h>

static void show(const char *s, int n) {
  for (int i = 0; i < n; ++i)
    std::putchar(s[i] ? s[i] : '.');
}

int main() {
  char a[] = "abcdefghij", b[] = "abcdefghij", c[] = "abcdefghij";
  char d[11] = {};
  bcopy(a, a + 2, 6);
  bcopy(b + 2, b, 6);
  bzero(c + 3, 4);
  bcopy("0123456789", d, 10);
  show(a, 10), std::putchar(' '), show(b, 10), std::putchar(' ');
  show(c, 10), std::printf(" %s\n", d);
  char e[8] = {};
  char *end = static_cast<char *>(mempcpy(e, "abcd", 4));
  std::printf("%s %d\n", e, static_cast<int>(end - e));
}
