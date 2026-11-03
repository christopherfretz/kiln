// -ftrivial-auto-var-init-max-size (spec/codegen.md §7y): a variable
// larger than the limit is left unwritten, whatever its type or
// initializer; one at or under it is filled. A VLA is always filled: the
// limit applies only to a fixed-size type.
//@ args: -ftrivial-auto-var-init=pattern -ftrivial-auto-var-init-max-size=16
void use(void *);
int g();
struct NT {
  NT() {}
  int a[8];
};
void sizes(int n) {
  int small[4];
  int big[5];
  NT nt;
  int big_init[8] = {g()};
  long l;
  int a[n];
  use(small);
  use(big);
  use(&nt);
  use(big_init);
  use(&l);
  use(a);
}
