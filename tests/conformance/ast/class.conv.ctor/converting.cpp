//@ expect: ast
//@ oracle: clang
// [class.conv.ctor], [dcl.init.ref]/5.4.1: converting constructors in
// reference binding and argument passing, and a conversion function
// feeding a converting constructor.
struct A { A(int); };
struct B { operator A(); };
struct C { C(B); };
void g(const A&);
void h(C);
void t(B b) {
  const A& r = 1;
  g(1);
  const A& r2 = b;
  h(b);
  C c = b;
}
