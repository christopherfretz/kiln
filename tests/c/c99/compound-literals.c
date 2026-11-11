// C17 6.5.2.5p4-p5: a compound literal is an lvalue; at file scope it
// has static storage duration, inside a function automatic storage
// associated with the enclosing block.
//@ expect: run
//@ xfail(kiln): WP-1276 follow-up, a compound literal is a prvalue in kiln (C17 6.5.2.5p4 makes it an lvalue): & of one and assignment to one are rejected
//@ stdout: 6 10 3 42
int printf(const char *, ...);
struct P { int x, y; };
int *fp = (int[]){ 1, 2, 3 };
struct P *gp = &(struct P){ 4, 6 };
int sum(const int *a, int n) { int s = 0; for (int i = 0; i < n; i++) s += a[i]; return s; }
int main(void) {
  int s = sum((int[]){ 1, 2, 3 }, 3);
  (struct P){ 1, 2 }.x = 5;
  int *p = &(int){ 41 };
  *p += 1;
  printf("%d %d %d %d\n", s, gp->x + gp->y, fp[2], *p);
  return 0;
}
