// [over.match.conv]: conversion functions initializing a non-class
// object; the best second standard conversion decides; explicit ones
// only in direct-initialization and contextual conversions.
//@ expect: compile-ok
struct A {
  operator int();
  operator double();
};
A a();
int i = a();
double d = a();
struct B {
  explicit operator bool();
  operator int *();
};
B b();
bool x(b());
int *p = b();
void t() {
  if (b()) {
  }
  while (b()) {
  }
  bool y = !b();
  bool z = b() && true;
}
struct C {
  operator int() const;
  operator long();
};
const C cc();
C c();
long l1 = cc();
long l2 = c();
struct D : A {};
D dd();
double e = dd();
