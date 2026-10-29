// [expr.dynamic.cast]/5-9: downcasts and upcasts through pointers, lvalue and
// rvalue references (a prvalue class operand is materialized for '&&'), and
// adding const are all well-formed.
//@ expect: compile-ok
//@ strict-diags
struct A { virtual void f(); };
struct B : A {};
A* pa; B* pb; A& ra = *pa; B mkb();
void g() {
  B* b1 = dynamic_cast<B*>(pa);
  A* a1 = dynamic_cast<A*>(pb);
  B&& r3 = dynamic_cast<B&&>(mkb());
  const A& r5 = dynamic_cast<const A&>(*pb);
  B& r1 = dynamic_cast<B&>(ra);
}
