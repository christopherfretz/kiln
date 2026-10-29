// [over.match.conv]: ambiguous and missing conversion functions.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:9 error "conversion from 'A' to 'short' is ambiguous"
//@ diag: 14:3 note "candidate function"
//@ diag: 15:3 note "candidate function"
//@ diag: 23:7 error "no viable conversion from 'B' to 'int'"
//@ diag: 18:12 note "explicit conversion function is not a candidate"
//@ diag: 24:7 error "no viable conversion from 'E' to 'int'"
//@ diag: 25:9 error "conversion from 'A' to 'float' is ambiguous"
//@ diag: 14:3 note "candidate function"
//@ diag: 15:3 note "candidate function"
struct A {
  operator int();
  operator long();
};
struct B {
  explicit operator int();
};
struct E {};
void t(A a, B b, E e) {
  short s = a;
  int i = b;
  int j = e;
  float f = a;
}
