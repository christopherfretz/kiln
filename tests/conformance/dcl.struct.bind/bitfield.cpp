//@ expect: ast
//@ ast-filter: test
// [dcl.struct.bind]/5 (C++26 /7): a binding to a bit-field member is a
// bit-field lvalue. Binding a const reference to it ([dcl.init.ref]/5.3)
// copies the value into a temporary, as for the member itself.
struct BF { int a : 3; int b; };
int test_bitfield(BF bf) {
  auto &[x, y] = bf;
  const int &c = x;
  const int &d = bf.a;
  x = 2;
  return c + d + x + y;
}
