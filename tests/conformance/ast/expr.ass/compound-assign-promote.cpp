//@ expect: ast
//@ oracle: clang
// [expr.ass]/6, [expr.arith.conv]/1, [conv.prom]: `E1 op= E2` computes
// `E1 op E2`, so the left operand's type takes part in the usual
// arithmetic conversions promoted (ComputeLHSTy), including the bit-field
// promotion of [conv.prom]/5. As in clang, two operands of the same
// unpromoted type keep it.
struct B {
  unsigned c : 9;
  int s : 7;
  unsigned w : 32;
  int iw : 32;
  long l3 : 3;
  unsigned long ul : 40;
  bool b : 1;
  unsigned short us : 5;
  char32_t c32 : 5;
};
void f(B *p, char32_t x, short sh, bool bb, unsigned u) {
  p->c += 2;
  p->c += 2u;
  p->c -= u;
  p->c *= 2.0;
  p->c &= 3;
  p->c <<= 1;
  p->s += 1;
  p->w += 1;
  p->iw += 1;
  p->l3 += 1;
  p->l3 += 1L;
  p->ul += 1;
  p->b += 1;
  p->b += true;
  p->us += 1;
  p->c32 += 1;
  x += 1;
  sh += sh;
  sh += 1;
  bb += bb;
}
