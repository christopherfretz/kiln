// C17 6.5.15p5, 6.5.3.2p4: void-typed `*p` operands of `?:`, `,`, a cast
// to `void` and a statement expression are evaluated for effect only;
// clang emits no load for them, `volatile void` included (WP-1347,
// gcc.c-torture compile/20000701-1.c).
//@ std: c17
//@ args: -w
//@ expect: ir
//@ min-oracle: 22
void f(void *pv, int i) { i ? *pv : *pv; }
void g(volatile void *pv, const void *pc, int i) {
  i ? *pv : *pc;
  i ? (i ? *pv : *pv) : (void)0;
  i ? (i ? *pv : *pc) : *pv;
  i ? *pv : (*pv, *pc);
  *pv, *pc;
  (void)*pv;
  *pv;
  ({ *pv; });
}
