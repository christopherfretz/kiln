// [expr.cond]/6: when overload resolution among the built-in conditional
// candidates is ambiguous the program is ill-formed; with no viable
// candidate the operands are incompatible.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:12 error "conditional expression is ambiguous; 'A' and 'B' can be converted to several common types"
//@ diag: 16:12 error "incompatible operand types ('C' and 'int *')"
struct A { operator int(); operator double(); };
struct B { operator int(); operator double(); };
struct C { };
struct OK { operator int *(); };

int f(bool b, A a, B x, C c, int *p, OK k) {
  (void)(b ? k : p);
  (void)(b ? a : x);
  (void)(b ? c : p);
  return 0;
}
