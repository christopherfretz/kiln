// C17 6.8.6.1p1, 6.8.4.2p2: a goto or a switch case label may not jump into
// the scope of an identifier with variably modified type (a variable length
// array, a VLA typedef); jumping out of one is fine (its stack is restored).
// WP-1286 owns the C rule for jumping past an ordinary initializer.
//@ std: c17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:3 error "cannot jump from switch statement to this case label"
//@ diag: 21:17 note "jump bypasses initialization of VLA typedef"
//@ diag: 13:3 error "cannot jump from this goto statement to its label"
//@ diag: 15:9 note "jump bypasses initialization of variable length array"
void f(int n, int k) {
  goto in;
  {
    int v[n];
  in:
    v[0] = 1;
  }
  switch (k) {
  case 0: {
    typedef int T[n];
  case 1:
    (void)sizeof(T);
  }
  }
  {
    int w[n];
    w[0] = 0;
    goto out;
  }
out:;
}
