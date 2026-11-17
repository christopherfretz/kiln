// [namespace.udecl]/11, [basic.scope.scope]/4.2: a class or enumeration does
// not correspond to a variable, a data member, an enumerator, a function or a
// function template, so it may be declared in the scope of a using-declarator
// naming one (the name then finds the non-type, an elaborated-type-specifier
// the class); a typedef-name or class declared first, then a using-declarator
// naming the same entity, is a redeclaration; an elaborated-type-specifier
// after a using-declarator naming a class refers to that class. In a class a
// member function hides a function the using-declarator named, and a data
// member beside a class the using-declarator named is valid. clang 22 and
// GCC 16.1 accept all of it.
//@ expect: compile-ok
//@ strict-diags
namespace N {
  int a;
  void b();
  template<class T> void c(T);
  enum { d };
  struct S {};
  typedef S T;
}
namespace M {
  using N::a; struct a {};
  using N::b; enum b { x };
  using N::c; struct c;
  using N::d; union d {};
  using N::S; struct S *p;
  using N::T; typedef N::S T;
}
void f() {
  using N::a; struct a {};
  using N::S; struct S *q = 0;
  int S = 0;
  struct a *r = 0;
  (void)q; (void)r; (void)S;
}
struct H {
  void g();
  struct C {};
  int v;
};
struct J : H {
  using H::g; void g();
  using H::C; int C;
  using H::v; struct v {};
};
int main() { struct M::a *s = 0; (void)s; }
