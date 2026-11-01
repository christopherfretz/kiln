//@ expect: ast
//@ ast-filter: f
// [expr.const.cast]/3: the class prvalue is materialized
// (MaterializeTemporaryExpr, after the CXXBindTemporaryExpr of a class
// with a non-trivial destructor) and the cast is a NoOp xvalue.
struct P { int x; };
struct D { int x; ~D(); };
P mk();
D mkd();
void f() {
  (void)const_cast<const P &&>(mk());
  (void)const_cast<P &&>(mk());
  (void)(const D &&)mkd();
  (void)const_cast<const D &&>(mkd());
}
