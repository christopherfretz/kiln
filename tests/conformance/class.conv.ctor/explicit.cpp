// [class.conv.ctor]/2, [dcl.fct.spec]/3: 'explicit' only on a constructor or
// conversion function, and only inside the class definition.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:1 error "'explicit' can only be specified inside the class definition"
//@ diag: 16:3 error "'explicit' can only appear on non-static member functions"
//@ diag: 17:3 error "'explicit' can only be applied to a constructor or conversion function"
//@ diag: 20:1 error "'explicit' can only be specified inside the class definition"
struct E {
  explicit E(int);
  explicit E();
  E(double);
};
explicit E::E(int) {}
struct F {
  explicit int x;
  explicit void f();
  explicit F(const F &);
};
explicit void g();
