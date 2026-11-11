// C89 3.1.2.6 with clang's library builtins: a declaration of a library
// function's name with an incompatible type, after an implicit call, is a
// new ordinary function (clang `MergeFunctionDecl` warns
// -Wincompatible-library-redeclaration); a later call calls it rather
// than folding as the library function, while the earlier call folds.
//@ std: c89
//@ expect: run
//@ args: -w
//@ stdout: -1 7
int printf(const char *, ...);
static int f(void) { return strncmp("a", "b", 1) < 0 ? -1 : 1; }
int strncmp(const char *a, const char *b, int n) { (void)a; (void)b; return n + 6; }
static int g(void) { return strncmp("a", "b", 1); }
int main(void) {
  printf("%d %d\n", f(), g());
  return 0;
}
