// [expr.ref]/1, [expr.prim.id.dtor]/2, [expr.call]/5: destructor names in a
// member access must name the object's type; a pseudo-destructor needs a
// scalar object, takes no arguments and must be called.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:6 error "destructor type 'B' in object destruction expression does not match the type 'A' of the object being destroyed"
//@ diag: 19:8 note "type 'B' found by destructor name lookup"
//@ diag: 25:4 error "member reference type 'A' is not a pointer; did you mean to use '.'?"
//@ diag: 26:5 error "member reference type 'A *' is a pointer; did you mean to use '->'?"
//@ diag: 29:6 error "expected a class name after '~' to name a destructor"
//@ diag: 30:6 error "the type of object expression ('int') does not match the type being destroyed ('A') in pseudo-destructor expression"
//@ diag: 31:3 error "call to pseudo-destructor cannot have any arguments"
//@ diag: 32:6 error "'Undeclared' does not refer to a type name in pseudo-destructor expression; expected the name of type 'int'"
//@ diag: 33:6 error "undeclared identifier 'Undeclared' in destructor name"
//@ diag: 34:6 error "member access into incomplete type 'Inc'"
//@ diag: 21:8 note "forward declaration of 'Inc'"
//@ diag: 35:3 error "reference to pseudo-destructor must be called; did you mean to call it with no arguments?"
struct A { ~A(); };
struct B {};
typedef int I;
struct Inc;
void f(A a, A* pa, int i, int* pi, B b, Inc* inc, I* pI) {
  a.~B();
  pa->~A();
  a->~A();
  pa.~A();
  i.~I();
  pi->~I();
  i.~float();
  i.~A();
  pi->~I(1);
  i.~Undeclared();
  a.~Undeclared();
  inc->~Inc();
  pI->~I;
}
