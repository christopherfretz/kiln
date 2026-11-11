int printf(const char *, ...);
struct s { int i; };
struct P { int x, y; };
union U { int i; unsigned char b[4]; };
struct Q { int *p; struct P pt; };
static struct P *gp = &(struct P){ 3, 4 };
static int *gi = &(int){ 9 };
static const char *gs = (const char[]){ 'o', 'k', 0 };
/* C17 6.5.2.5p15 (EXAMPLE 8): one object per literal, re-initialized
   each time its block is entered or the literal is evaluated. */
__attribute__((noinline)) int same_object(void) {
  struct s *p = 0, *q;
  int j = 0;
again:
  q = p, p = &((struct s){ j++ });
  if (j < 2) goto again;
  return p == q && q->i == 1;
}
__attribute__((noinline)) int by_value(struct P p) { return p.x * 10 + p.y; }
__attribute__((noinline)) int loop_sum(int n) {
  int t = 0;
  for (int i = 0; i < n; i++) {
    int *a = (int[3]){ i, i + 1 };
    a[2] += a[0] + a[1];
    t += a[2];
  }
  return t;
}
int main(void) {
  struct P *pp = &(struct P){ 1, 2 };
  pp->y = 7;
  (struct P){ 5, 6 }.x = 0;
  ((struct P){ 5, 6 }) = (struct P){ 8, 9 };
  int *ip = &(int){ 10 };
  ++*ip;
  *ip += 1;
  (int){ 1 }++;
  union U *u = &(union U){ .i = 0 };
  u->b[0] = 1;
  struct Q *q = &(struct Q){ &(int){ 3 }, { 2, 1 } };
  *q->p *= 2;
  printf("%d %d %d %d %d | ", pp->x, pp->y, *ip, u->i != 0, *q->p + q->pt.x);
  gp->x += 1;
  *gi += 1;
  printf("%d %d %s %d %d %d | ", gp->x + gp->y, *gi, gs, same_object(),
         by_value((struct P){ 4, 2 }), loop_sum(4));
  printf("%d %d\n", (int)sizeof (struct P){ 0 }, (int)sizeof (int[]){ 1, 2, 3 });
  return 0;
}
