// [basic.indet]/1-2 (C++26, P2795R5): an automatic variable without an
// initializer gets kiln's erroneous value, all-zero bytes, written at its
// declaration (spec/codegen.md §7r): a zero store for a scalar, an
// `llvm.memset` for anything else (arrays, classes, padding included),
// each tagged `!annotation !{!"auto-init"}`. That is the IR clang emits
// under `-ftrivial-auto-var-init=zero`, which the `args` line gives both
// compilers (the clang oracles have no erroneous values, and kiln's
// comparison flags pin `=uninitialized`). Above 4096 bytes nothing is
// written (`int[1025]`), like clang's
// `-ftrivial-auto-var-init-max-size=4096`. A class whose default
// constructor runs and `[[indeterminate]]` variables are not filled;
// kiln-only (clang fills both), so they are tested in
// crates/kiln_driver/tests/erroneous_init.rs.
//@ args: -ftrivial-auto-var-init=zero -ftrivial-auto-var-init-max-size=4096
void use(void *);
struct Padded {
  char c;
  int i;
  short s;
};
struct Empty {};
struct WithDtor {
  int v;
  ~WithDtor();
};
int scalar() {
  int x;
  use(&x);
  return x;
}
void scalars() {
  double d;
  bool b;
  void *p;
  volatile int v;
  use(&d);
  use(&b);
  use(&p);
  use((void *)&v);
}
void aggregates() {
  int a[16];
  Padded s;
  Empty e;
  WithDtor w;
  use(a);
  use(&s);
  use(&e);
  use(&w);
}
void threshold() {
  int at[1024];
  int above[1025];
  use(at);
  use(above);
}
void nested(int n) {
  for (int i = 0; i < n; ++i) {
    int t;
    use(&t);
  }
}
