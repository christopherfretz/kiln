// [over.match.oper]/3.4.4, P2468: ambiguous reversed candidates,
// -Wambiguous-reversed-operator, deleted and implicit assignment.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:12 warning "ISO C++20 considers use of overloaded operator '==' (with operand types 'A' and 'A') to be ambiguous despite there being a unique best viable function"
//@ diag: 12:17 note "ambiguity is between a regular call to this operator and a call with the argument order reversed"
//@ diag: 12:17 note "mark 'operator==' as const or add a matching 'operator!=' to resolve the ambiguity"
//@ diag: 25:5 error "overload resolution selected deleted operator '='"
//@ diag: 19:15 note "candidate function has been explicitly deleted"
//@ diag: 26:5 error "object of type 'H' cannot be assigned because its copy assignment operator is implicitly deleted"
//@ diag: 20:17 note "copy assignment operator of 'H' is implicitly deleted because field 'r' is of reference type 'int &'"
struct A { bool operator==(const A&); };
struct B {};
struct C { bool operator==(const B&) const; };
struct D { operator B() const; };
bool operator==(const D&, const C&);
bool operator==(B, C);
struct E { bool operator==(E) const; bool operator!=(E) const; };
struct G { G& operator=(const G&) = delete; G(); };
struct H { int &r; };
void f(A a, C c, D d, E e, G g, H h, H h2) {
  (void)(a == a);
  (void)(c == d);
  (void)(e == e);
  g = g;
  h = h2;
}
