// C17 6.7.4p7: an inline definition (all file-scope declarations inline,
// none extern) provides no external definition; the translation unit
// that also declares the function extern (or without inline) emits it.
// So this unit references `sq` and the other unit defines it once; the
// static inline function is emitted locally.
//@ expect: run
//@ unit: Inputs/inline-c99-def.c
//@ stdout: 49 9 2
int printf(const char *, ...);
inline int sq(int x) { return x * x; }
static inline int dbl(int x) { return 2 * x; }
int other(void);
int main(void) {
  int (*p)(int) = sq;
  printf("%d %d %d\n", p(7), sq(3), dbl(1) + other() - 1);
  return 0;
}
