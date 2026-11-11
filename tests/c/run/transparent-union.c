// GNU transparent_union in C (clang `handleTransparentUnionAttr` and
// `CheckTransparentUnionArgumentConstraints`, WP-1287): written after the
// class-key, after the closing brace, or on a typedef of a complete union
// (glibc's `__SOCKADDR_ARG`), an argument for a parameter of the union
// initializes the first member it is assignment-compatible with (a pointer
// member also takes a `void *` and a null pointer constant), and the union
// passes as that first member.
//@ std: c17
//@ expect: run
//@ stdout: 5 6 -1 5 | 11 11 | 7 1 | 5 str
int printf(const char *, ...);
typedef union { int *ip; const long *lp; void *vp; } __attribute__((transparent_union)) arg_t;
struct sa { int x; };
typedef union { struct sa *__restrict p; const struct sa *__restrict cp; } SA __attribute__((__transparent_union__));
union __attribute__((transparent_union)) U2 { __INTPTR_TYPE__ l; unsigned *u; };
union U3 { __INTPTR_TYPE__ i; const char *s; } __attribute__((transparent_union));
__attribute__((noinline)) static int f(arg_t a) { return a.vp ? *a.ip : -1; }
__attribute__((noinline)) static int g(SA a) { return a.cp->x; }
__attribute__((noinline)) static long h(union U2 a) { return (long)a.l; }
__attribute__((noinline)) static const char *k(union U3 a) { return a.s; }
int main(void) {
  int i = 5;
  long l = 6;
  unsigned u = 9;
  struct sa s = { 11 };
  void *vp = &i;
  printf("%d %d %d %d | ", f(&i), f(&l), f(0), f(vp));
  printf("%d %d | ", g(&s), g((const struct sa *)&s));
  printf("%ld %d | ", h(7), (int)(h(&u) == (long)(__INTPTR_TYPE__)&u));
  arg_t a = { &i };
  printf("%d %s\n", f(a), k("str"));
  return 0;
}
