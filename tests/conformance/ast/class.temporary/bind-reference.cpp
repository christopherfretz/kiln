//@ expect: ast
//@ ast-filter: f
// [dcl.init.ref]/5.3.1, 5.4.2, [class.temporary]/4, /6, [conv.rval]:
// references bound to temporaries. A reference-compatible prvalue is
// adjusted to the reference's cv-qualification, materialized and extended
// by the variable (then converted to the base); a non-class value of an
// unrelated type is converted to a temporary first. A class prvalue with a
// non-trivial destructor is bound (CXXBindTemporaryExpr); a temporary
// that is only accessed is materialized without extension. Matches clang 19.
namespace q {
struct S { int a; ~S(); };
struct D : S {};
struct T { int a; };
S mk(); D mkd(); T mkt();
void f() {
  const int &r1 = 1;
  const long &r2 = 1;
  int &&r3 = 2;
  const S &r4 = mk();
  const S &r5 = mkd();
  S &&r6 = mk();
  const T &r7 = mkt();
  int x = mkt().a;
  static const int &r8 = 3;
}
const int &g = 4;
}
