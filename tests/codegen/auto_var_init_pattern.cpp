// -ftrivial-auto-var-init=pattern (spec/codegen.md §7y): clang's
// initialization pattern. Integers and padding are 0xAA bytes, pointers
// are inttoptr of 0xAAAA..., floating point is all-ones (a NaN). A fill
// larger than 32 bytes that is one repeated byte is a memset; one with
// few non-zero stores is a memset 0 plus stores; anything else is a
// memcpy from a `__const.<fn>.<var>` global. Every fill store is marked
// `!annotation !{!"auto-init"}`.
//@ args: -ftrivial-auto-var-init=pattern
void use(void *);
int g();
struct E {};
struct Pd {
  char c;
  int i;
  short s;
};
union U {
  char c;
  long l;
};
struct NT {
  NT() {}
  int a;
  char b;
};
struct Mp {
  int m;
  void f();
};
void scalars() {
  char c;
  short s;
  int i;
  long l;
  __int128 w;
  bool b;
  int *p;
  decltype(nullptr) n;
  float f;
  double d;
  long double ld;
  _BitInt(37) bi;
  int Mp::*dm;
  void (Mp::*fm)();
  _Complex float cf;
  use(&c);
  use(&s);
  use(&i);
  use(&l);
  use(&w);
  use(&b);
  use(&p);
  use(&n);
  use(&f);
  use(&d);
  use(&ld);
  use(&bi);
  use(&dm);
  use(&fm);
  use(&cf);
}
void aggregates() {
  E e;
  Pd pd;
  U u;
  int big[20];
  float fl[40];
  Pd many[8];
  long double l3[3];
  NT nt;
  NT nts[2];
  use(&e);
  use(&pd);
  use(&u);
  use(big);
  use(fl);
  use(many);
  use(l3);
  use(&nt);
  use(nts);
}
void initialized() {
  int i = g();
  Pd pd{1, g(), 3};
  Pd k{1, 2, 3};
  int a[3] = {g(), 1, 2};
  use(&i);
  use(&pd);
  use(&k);
  use(a);
}
void vla(int n) {
  int a[n];
  char c[n];
  Pd p[n];
  use(a);
  use(c);
  use(p);
}
void unmarked() {
  [[clang::uninitialized]] int x;
  [[clang::uninitialized]] Pd pd;
  use(&x);
  use(&pd);
}
