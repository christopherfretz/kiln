// [class.protected]/1: a protected non-static member of a base is usable
// from a derived class only through an object of that derived class (or a
// class derived from it).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:60 error "'x' is a protected member of 'B'"
//@ diag: 14:7 note "can only access this member on an object of type 'D1'"
//@ diag: 18:68 error "'x' is a protected member of 'B'"
//@ diag: 14:7 note "can only access this member on an object of type 'D1'"
//@ diag: 21:44 error "'x' is a protected member of 'B'"
//@ diag: 14:7 note "can only access this member on an object of type 'D2'"
struct B {
protected:
  int x;
  static int s;
};
struct D1 : B {
  int f(B &b, D1 &d, B *pb) { return d.x + x + s + b.s + b.x + pb->x; }
};
struct D2 : D1 {
  int g(D1 &d1, D2 &d2) { return d2.x + d1.x; }
};
