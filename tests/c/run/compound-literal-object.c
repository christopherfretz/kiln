// C17 6.5.2.5p4-p5: a compound literal is its own object. Assigned to a
// structure it is built first and then copied, so its initializers read
// the target's old value; of array type it decays to a pointer to its
// first element, which stays valid until the enclosing block ends.
//@ std: c17
//@ expect: run
//@ args: -O2
//@ stdout: 12 6 0 7 9 | 0 82 82 3
int printf(const char *, ...);
int memcmp(const void *, const void *, unsigned long);
struct T { int t; int r[4]; };
struct S { int a; int b; int c[3]; struct T d; };
typedef const unsigned short *P;
__attribute__((noinline)) void swap(struct S *s) {
  *s = (struct S){ s->b, s->a, { 0, 0, 0 }, s->d };
}
__attribute__((noinline)) int first(P p) { return p[0]; }
int main(void) {
  struct S s = { 6, 12, { 1, 2, 3 }, { 7, { 8, 9, 10, 11 } } };
  swap(&s);
  printf("%d %d %d %d %d | ", s.a, s.b, s.c[0] + s.c[1] + s.c[2], s.d.t, s.d.r[1]);
  const struct T f[] = { { 1, { 2, 3, 0, 0 } } };
  const P q = (const unsigned short[]){ 'R' };
  const int *n = (int[]){ 1, 2, 3 };
  printf("%d %d %d %d\n", memcmp(f, (struct T[]){ { 1, { 2, 3 } } }, sizeof(struct T)),
         first(q), q[0], n[2]);
  return 0;
}
