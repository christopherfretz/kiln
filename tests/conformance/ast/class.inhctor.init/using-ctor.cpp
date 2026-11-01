//@ expect: ast
// [namespace.udecl]/1, /3, [class.inhctor.init]/1: `using B::B;` is a
// UsingDecl named `B::D` (clang names it for the derived class) with one
// ConstructorUsingShadowDecl per base constructor; in a template with a
// dependent base it is an UnresolvedUsingValueDecl named by the canonical
// base type; a construction of the derived class from base arguments
// calls the implicit inheriting constructor. Matches clang 19.
struct B { B(int, int); B(double); };
struct D : B { using B::B; };
template <class T> struct Base { Base(T); };
template <class T> struct E : Base<T> { using Base<T>::Base; };
template <class T> struct F : T { using T::T; };
void use() {
  D d(1, 2);
  D d2(1.0);
}
