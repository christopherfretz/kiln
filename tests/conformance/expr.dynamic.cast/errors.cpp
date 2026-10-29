// [expr.dynamic.cast]/1-9: the target is a pointer or reference to a complete
// class (or cv void*); the operand matches (pointer, lvalue for '&'); constness
// cannot be cast away; an upcast is checked like an implicit conversion (access,
// ambiguity); a downcast or cross-cast requires a polymorphic source class.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 38:11 error "dynamic_cast from 'const A *' to 'B *' casts away qualifiers"
//@ diag: 39:11 error "ambiguous conversion from derived class 'D' to base class 'A':"
//@ diag: 40:11 error "'P' is not polymorphic"
//@ diag: 42:11 error "'X' is an incomplete type"
//@ diag: 29:8 note "forward declaration of 'X'"
//@ diag: 43:11 error "'X' is an incomplete type"
//@ diag: 29:8 note "forward declaration of 'X'"
//@ diag: 44:13 error "'int' is not a class type"
//@ diag: 45:11 error "'int' is not a class type"
//@ diag: 49:11 error "dynamic_cast from rvalue to reference type 'B &'"
//@ diag: 50:11 error "cannot use dynamic_cast to convert from 'A' to 'A *'"
//@ diag: 51:11 error "invalid target type 'int' for dynamic_cast; target type must be a reference or pointer type to a defined class"
//@ diag: 52:11 error "cannot cast 'Priv' to its private base class 'A'"
//@ diag: 30:15 note "declared private here"
//@ diag: 55:11 error "'A *' is not a class type"
//@ diag: 56:14 error "'P' is not polymorphic"
struct A { virtual void f(); };
struct B : A {};
struct C : A {};
struct D : B, C {};
struct P { int x; };
struct Q : P {};
struct X;
struct Priv : private A {};
A* pa; B* pb; D* pd; const A* cpa; P* pp; Q* pq; X* px; int* pi; Priv* ppriv;
A& ra = *pa; B mkb();
void f() {
  B* b1 = dynamic_cast<B*>(pa);
  A* a1 = dynamic_cast<A*>(pb);
  void* v1 = dynamic_cast<void*>(pa);
  const void* v2 = dynamic_cast<const void*>(cpa);
  B* b2 = dynamic_cast<B*>(cpa);
  A* a2 = dynamic_cast<A*>(pd);
  Q* q1 = dynamic_cast<Q*>(pp);
  P* p1 = dynamic_cast<P*>(pq);
  X* x1 = dynamic_cast<X*>(pa);
  A* a3 = dynamic_cast<A*>(px);
  int* i1 = dynamic_cast<int*>(pa);
  A* a4 = dynamic_cast<A*>(pi);
  B& r1 = dynamic_cast<B&>(ra);
  B& r2 = dynamic_cast<B&>(*pa);
  B&& r3 = dynamic_cast<B&&>(ra);
  B& r4 = dynamic_cast<B&>(mkb());
  A* a5 = dynamic_cast<A*>(ra);
  int n = dynamic_cast<int>(ra);
  A* a6 = dynamic_cast<A*>(ppriv);
  A* a7 = dynamic_cast<A*>(pa);
  const A& r5 = dynamic_cast<const A&>(*pb);
  B& r6 = dynamic_cast<B&>(pa);
  void* v3 = dynamic_cast<void*>(pp);
}
