// [expr.ass]/6, [conv.prom]/3: the left operand of `E1 op= E2` takes part
// in the computation promoted, so for an unscoped enumeration the result
// is an integer (or floating) value that cannot be assigned back
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:5 error "assigning to 'E' from incompatible type 'int'"
//@ diag: 17:5 error "assigning to 'E' from incompatible type 'int'"
//@ diag: 18:5 error "assigning to 'E' from incompatible type 'int'"
//@ diag: 19:5 error "assigning to 'E' from incompatible type 'double'"
//@ diag: 20:5 error "assigning to 'F' from incompatible type 'int'"
//@ diag: 21:7 error "assigning to 'E' from incompatible type 'int'"
enum E { E0, E1 };
enum F : unsigned char { F0 };
struct B { E e : 2; };
void g(E e, F f, B b) {
  e += 1;
  e |= E1;
  e <<= 1;
  e += 1.0;
  f += F0;
  b.e %= 2;
}
