//@ expect: ast
//@ ast-filter: f
// [dcl.init.aggr]/2, /3.1, /5.1, /16: the elements of an aggregate with
// base classes are its direct bases in declaration order, then its
// fields. A base takes a nested list, a brace-elided run of the list or an
// expression of its class (a prvalue moved from: no elision into a base
// subobject); designated initializers skip the bases, which are then
// value-initialized. Matches clang 19.
struct B { int b; };
struct C { int c = 7; };
struct P : B, C { int x = 5; long y; int z; };
struct D : P { int w; };
P mk();
void f() {
  P r{{1}, {2}, 3, 4L, 5};
  P s{.x = 1, .z = 3};
  P t{B{1}};
  P u{1, 2};
  D d{mk(), 9};
  D e{};
}
