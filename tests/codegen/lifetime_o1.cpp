//@ args: -O1
// [basic.life], [stmt.dcl]/2-3, [class.temporary]/4: lifetime markers at
// -O1 in every function (clang ShouldEmitLifetimeMarkers). Each automatic
// variable gets llvm.lifetime.start after its alloca and llvm.lifetime.end
// when its scope ends (also on the EH path); a materialized temporary is
// covered for its full-expression. A variable a `goto` or `switch` jumps
// past gets none (VarBypassDetector). `optnone` keeps the markers, which
// clang emits there too.
#define ON __attribute__((optnone))
struct S { int a[4]; ~S(); };
struct P { int a[4]; };
void use(int *);
void usep(const P &);
P mk();
int may_throw(int);
ON int scopes(int n) {
  int x = n;
  use(&x);
  {
    int y[8];
    use(y);
  }
  if (int z = n - 1) {
    use(&z);
  }
  return x;
}
ON void temps(int n) {
  usep(P{{n, n, n, n}});
  usep(mk());
  usep(n ? P{{1, 2, 3, 4}} : mk());
  const P &r = mk();
  usep(r);
}
ON void eh(int n) {
  S s;
  int k = may_throw(n);
  use(&k);
}
ON int bypass_goto(int n) {
  if (n) goto out;
  {
    int skipped;
    use(&skipped);
  out:;
  }
  int after = n;
  use(&after);
  return after;
}
ON int bypass_switch(int n) {
  switch (n) {
    int in_switch;
  case 0:
    in_switch = 1;
    use(&in_switch);
    return in_switch;
  case 1: {
    int scoped = 2;
    use(&scoped);
    return scoped;
  }
  }
  return 0;
}
// Not optnone: the optimizer keeps the markers around the escaping local.
int opt(int n) {
  int v = n;
  use(&v);
  return v;
}
