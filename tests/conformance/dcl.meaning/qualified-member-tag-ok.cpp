// [dcl.meaning.general]/2: a nested class declared in its class can be
// defined outside it with a qualified name, at namespace scope; a friend
// declaration may name a class through a qualifier.
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
namespace N { struct z { int v = 1; }; }
struct A {
  struct B;
  enum class E;
  template <class T> struct M;
  friend struct N::z;
};
struct A::B { struct C; };
struct A::B::C { int w = 2; };
enum class A::E { e = 3 };
template <class T> struct A::M { T t = 4; };
int use() { return N::z().v + A::B::C().w + int(A::E::e) + A::M<int>().t; }
