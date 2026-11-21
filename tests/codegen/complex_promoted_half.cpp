// WP-1795: -fcomplex-arithmetic=promoted on `_Complex _Float16`. A
// quotient by a complex divisor runs algebraically in `float`; the
// other arithmetic keeps x86-64's excess-precision `float` (spec/abi.md),
// where a negated or unary-plus operand is emitted promoted too (clang
// `VisitUnaryMinus` with a promotion type).
//@ args: -fcomplex-arithmetic=promoted
typedef _Complex _Float16 ch;
ch divh(ch a, ch b) { return a / b; }
ch mulh(ch a, ch b) { return a * b; }
ch divh_real(ch a, _Float16 b) { return a / b; }
ch chainh(ch a, ch b, ch c) { return a / (b * c) / -c; }
ch negh(ch a) { return -a; }
ch plush(ch a) { return +a; }
ch negsum(ch a, ch b) { return -(a + b); }
void compoundh(ch &a, ch b, _Float16 c) {
  a /= b;
  a *= b;
  a /= c;
}
