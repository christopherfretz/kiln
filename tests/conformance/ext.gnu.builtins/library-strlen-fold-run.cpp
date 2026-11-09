// Library builtin calls folded by codegen (WP-940) keep their values:
// constant arguments fold, others call the library.
//@ expect: run
//@ std: c++20
//@ stdout: 4 5 -1 1 2 llo 3
#include <cstdio>
extern "C" {
__SIZE_TYPE__ strlen(const char *);
int strcmp(const char *, const char *);
int memcmp(const void *, const void *, __SIZE_TYPE__);
char *strchr(const char *, int);
}
static const char hello[] = "hello";
__SIZE_TYPE__ global_len = strlen("abcd");
const char *global_chr = strchr(hello, 'l');
int main(int argc, char **) {
  int c = strcmp("a", "b") < 0 ? -1 : 1;
  int m = memcmp(hello, "help", 4) < 0;
  std::printf("%zu %zu %d %d %zu %s %zu\n", global_len, strlen(hello), c, m,
              strlen(argc > 5 ? "x" : "xy"), global_chr, strlen("abc"));
}
