//@ expect: ast
// [class.mem.general]/1: nested classes, mutable data members, bit-fields
// (named, unnamed, of enumeration type, several in one declaration) and
// empty declarations.
struct A final { int x; };
enum E { e0, e1 };
struct Outer {
  struct In { int z; } in;
  E e : 2;
  mutable int m;
  int : 0;
  unsigned bits : 3, more : 4;
  ;
};
int use(Outer o) { return o.in.z + o.bits; }
