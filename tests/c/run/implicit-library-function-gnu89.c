// C89 3.3.2.2 with clang's predefined library functions: an undeclared
// `alloca` is declared `void *(size_t)` (clang `LazilyCreateBuiltin`), so
// its pointer result is not truncated to an implicit `int`, and a later
// `int strlen();` is a warning, not "conflicting types" (gcc.c-torture
// execute/941202-1.c).
//@ args: -std=gnu89 -w
//@ expect: run
//@ stdout: 3 4
static int f(int i) {
  int *tmp = (int *)alloca(sizeof(i));
  *tmp = i;
  if (*tmp != 3) abort();
  return *tmp;
}
static int n(void) { return (int)strlen("abcd"); }
int strlen();
int main(void) {
  printf("%d %d\n", f(3), n());
  return 0;
}
