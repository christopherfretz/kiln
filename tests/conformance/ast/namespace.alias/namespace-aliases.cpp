//@ expect: ast
// [namespace.alias]: a namespace alias is a synonym for the namespace it
// names, also when it names another alias; it may be redeclared to denote
// the same namespace.
namespace A {
namespace B { int t; }
int y;
}
namespace AB = A::B;
namespace AB2 = AB;
namespace AB = A::B;
int a = AB::t + AB2::t;
int f() {
  namespace L = A;
  return L::y + L::B::t;
}
