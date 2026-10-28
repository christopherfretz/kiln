//@ expect: ast
struct P { int v; };
int addr(P p, P *pp) {
  int *a = &p.v;
  P *q = &p;
  int **aa = &a;
  return *a + (*q).v + **aa + (&pp->v)[0];
}
