// C17 6.3.2.3p1, 6.5.16.1p1, 6.5.4p3: a pointer to void converts to and
// from a pointer to any object type without a cast, in initialization,
// assignment, argument passing and return. A pointer to function does
// not (an extension, -Wpedantic only).
//@ expect: compile-ok
//@ strict-diags
void *malloc(unsigned long);
void free(void *);
struct S { int x; };
static struct S *make(void) { return malloc(sizeof(struct S)); }
static void take(int *p) { (void)p; }
int main(void) {
  int *ip = malloc(4);
  void *vp = ip;
  const void *cvp = ip;
  double *dp;
  dp = vp;
  take(vp);
  struct S *s = make();
  free(s);
  free(dp);
  (void)cvp;
  char **cpp = vp;
  (void)cpp;
  return 0;
}
