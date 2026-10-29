// [expr.typeid]/3-4: a glvalue of polymorphic class type is evaluated (dynamic
// type), other operands are unevaluated; both, and a type-id, are well-formed.
//@ expect: compile-ok
//@ strict-diags
namespace std { class type_info; }
struct A { virtual void f(); };
struct P { int x; };
A* pa; P* pp; A mka();
void g() {
  (void)typeid(*pa);
  (void)typeid(*pp);
  (void)typeid(mka());
  (void)typeid(A);
}
