// GNU89 inline semantics (-fgnu89-inline, as clang's -std=gnu89): a
// plain `inline` definition is an external definition, an `extern
// inline` one provides none (C99's meaning reversed);
// __GNUC_GNU_INLINE__ is defined.
//@ expect: run
//@ args: -fgnu89-inline
//@ unit: Inputs/inline-gnu89-def.c
//@ stdout: 27 1
int printf(const char *, ...);
extern inline int cube(int x) { return x * x * x; }
int main(void) {
  int (*p)(int) = cube;
#ifdef __GNUC_GNU_INLINE__
  int g = 1;
#else
  int g = 0;
#endif
  printf("%d %d\n", p(3), g);
  return 0;
}
