// C23 6.8.7.2p1 (the goto statement): a goto shall not jump from outside
// the scope of an identifier with variably modified type into it; jumping
// past an ordinary initialized object is fine (tests/c/stmt/goto-past-initializer.c).
// Shared with the VLA work (WP-1287): the rule lives in the jump checker.
//@ args: -std=gnu17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:3 error "cannot jump from this goto statement to its label"
//@ diag: 16:9 note "jump bypasses initialization of variable length array"
//@ diag: 23:3 error "cannot jump from this goto statement to its label"
//@ diag: 24:7 note "jump bypasses initialization of variable length array"
void use(int *);
void f(int n) {
  goto in;
  {
    int v[n];
  in:
    use(v);
  }
}
void g(int n) {
  int x = 1;
  goto skip;
  int a[n];
  x = a[0];
skip:
  use(&x);
}
