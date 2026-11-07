//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 17:28 error "'this' cannot be implicitly captured in this context"
//@ diag: 17:12 note "explicitly capture 'this'"
//@ diag: 18:27 error "'this' cannot be implicitly captured in this context"
//@ diag: 18:12 note "explicitly capture 'this'"
//@ diag: 19:26 error "'this' cannot be implicitly captured in this context"
//@ diag: 19:12 note "explicitly capture 'this'"
// Only a qualified name that is the whole operand of `&` forms a pointer
// to member; a parenthesized one or one with a postfix suffix is a
// member access that needs `this` ([expr.unary.op]/4).
struct H {
  int x = 0;
  int a[2] = {};
  void run() {
    (void)[] { return &(H::x); };
    (void)[] { return &H::a[0]; };
    (void)[] { return H::x; };
    (void)[] { return &H::x; };
  }
};
