//@ expect: ast
//@ oracle: clang
// [expr.static.cast]/4, [expr.cast]/4, [expr.type.conv]/2: explicit
// casts of a class to a non-class type by a conversion function
// (explicit ones included).
struct A { operator int() const; explicit operator long() const; operator double&(); };
void f(A a) {
  int i = static_cast<int>(a);
  long l = static_cast<long>(a);
  double d = (double)a;
  short s = short(a);
}
