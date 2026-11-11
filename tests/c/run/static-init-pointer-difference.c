// C23 6.6p9 (GNU C, as clang and GCC): in an initializer of an object with
// static storage duration, the difference of two addresses within one
// complete object is a constant even when they are not elements of one
// array (`s.f - &s.b`, `(char *)&v.p[0] - (char *)&v`), as is offsetof's
// classic form `(char *)&((T *)0)->m - (char *)0`.
//@ expect: run
//@ stdout: 1 1 0 1 | 8 8
#include <stdio.h>
struct { char a, b, f[3]; } s;
long i = s.f - &s.b;
long long j = s.f - &s.b;
struct s2 { char p[2]; };
static struct s2 v;
const int o0 = (int)((char *)&v.p[0] - (char *)&v) + 0U;
const int o1 = (int)((char *)&v.p[1] - (char *)&v);
struct S { long n; char name[10]; };
static unsigned long off[] = {
  (unsigned long)((unsigned char *)&((struct S *)0)->name - (unsigned char *)0),
};
static unsigned long off2 = (unsigned long)&((struct S *)0)->name;
int main(void) {
  printf("%ld %lld %d %d | %lu %lu\n", i, j, o0, o1, off[0], off2);
  return 0;
}
