// C89 3.3.2.2 with clang's predefined library functions (clang
// `LazilyCreateBuiltin`, Builtins.td `LibBuiltin`): an undeclared call of
// `exit`, `free`, `strtod`, `strtol` and the rest of clang's <stdlib.h> /
// <string.h> / <stdio.h> library builtins declares the function with its
// library type, not `int ()`, so `strtod` returns a `double` and `exit`
// is `noreturn` (gcc.c-torture calls them undeclared).
//@ std: c89
//@ expect: run
//@ args: -Wno-implicit-function-declaration
//@ stdout: 42 1.5 2 7 3
typedef struct _IO_FILE FILE;
extern FILE *stdout;
int printf(const char *, ...);
static int f(void) {
  exit(3);
}
int main(void) {
  char *e;
  long v = strtol("42", &e, 10);
  double d = strtod("1.5", 0);
  unsigned long n = strcspn("abc", "c");
  void *p = malloc(4);
  float g = strtof("7", 0);
  free(p);
  fwrite("", 1, 0, stdout);
  printf("%ld %.1f %lu %.0f ", v, d, n, (double)g);
  printf("%d\n", (int)sizeof(strtoull("1", 0, 10)) == (int)sizeof(unsigned long long) ? 3 : 0);
  return 0;
  (void)f;
}
