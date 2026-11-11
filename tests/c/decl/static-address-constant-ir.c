// C23 6.6p9, 6.7.11p4: address constants formed through casts are emitted
// as static data, as clang's constant emitter does: a `getelementptr` for an
// address plus an offset, `inttoptr` for an integer cast to a pointer,
// `ptrtoint` for an address cast to an integer as wide as a pointer, with
// no guard variable and no initialization function for the block-scope
// static.
//@ expect: ir
//@ min-oracle: 22
typedef __INTPTR_TYPE__ iptr;
struct S { int a, b; } gs = { 1, 2 };
int g = 3;
char arr[10] = "abc";
static int one(void) { return 1; }
char *p1 = (char *)&gs;
char *p2 = (char *)&arr[3];
void *p3 = (void *)one;
void *p4 = (void *)16;
int *p5 = (int *)&gs + 1;
iptr i1 = (iptr)&g;
struct { iptr x; void *p; } st = { (iptr)&gs.b, (void *)&arr[9] };
char *f(void) {
  static char *q = (char *)&gs.b + 1;
  return q;
}
