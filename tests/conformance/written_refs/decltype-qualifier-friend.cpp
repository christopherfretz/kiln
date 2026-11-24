// The written-reference table (spec/ast.md §18.4) for a friend declarator-id
// qualified by a decltype-specifier ([class.friend], [expr.prim.id.qual]/1):
// the decltype operand's references are recorded, plain and inside a class
// template whose operand names a dependent specialization. Reduced from real
// code.
//@ written-refs: decltype-qualifier-friend.refs
struct A { int peek(int); };
A a;
class K {
  friend int decltype(a)::peek(int);
};
template <int N> struct Bq { int g(); };
template <class T> class D {
  friend int decltype(Bq<sizeof(T)>{})::g();
};
D<int> d;
