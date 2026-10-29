//@ expect: ast
//@ oracle: clang
// [class.conv.fct], [over.match.conv], [over.match.ref]: conversion
// functions applied in copy-initialization of non-class and reference
// targets (a UserDefinedConversion cast over the member call).
struct A { int v; };
struct C { operator int() const; };
struct D { operator double(); };
struct R { operator int&(); };
struct B { operator A(); };
void t(C c, D d, R r, B b) {
  int i = c;
  int j = d;
  int k = r;
  const int& cr = c;
  int& lr = r;
  A a = b;
  long l = c;
}
