// Debug info (WP-1445): a label gets a DILabel at its statement and a
// `#dbg_label` record at the start of its block, as clang emits it; a
// label in dead code and labels reached by forward and backward gotos.
//@ args: -g
extern int g;
void f() {
  goto out;
dead:
  g = 1;
out:
  g = 2;
}
static inline void h() {
lbl:
  g = 3;
  if (g)
    goto lbl;
}
int k(int a) {
  h();
top:
  if (a--)
    goto top;
  return a;
}
