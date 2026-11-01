// [dcl.attr.assume]/1, [expr.comma] (WP-182): a comma inside an assumption
// is diagnosed for parentheses, and its left operand still gets
// -Wunused-value (clang `CheckCommaOperands`), also on instantiation. An
// unparenthesized comparison on the left is -Wunused-comparison with the
// `=` fix-it note. After an attribute, anything but `,` or `]` is
// "expected ','" after the previous token (clang
// `ParseCXX11AttributeSpecifierInternal`), then the rest is skipped to `]`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:13 error "use of this expression in an 'assume' attribute requires parentheses"
//@ diag: 22:12 warning "left operand of comma operator has no effect"
//@ diag: 23:13 warning "left operand of comma operator has no effect"
//@ diag: 24:14 warning "equality comparison result unused"
//@ diag: 24:14 note "use '=' to turn this equality comparison into an assignment"
//@ diag: 24:18 error "use of this expression in an 'assume' attribute requires parentheses"
//@ diag: 26:43 warning "left operand of comma operator has no effect"
//@ diag: 27:15 note "in instantiation of function template specialization 't<int>' requested here"
//@ diag: 29:11 error "expected ','"
//@ diag: 29:5 error "'assume' attribute takes one argument"
//@ diag: 32:14 error "expected ','"
void f(int x, int y) {
  [[assume(x, y > 0)]];
  [[assume((x, y > 0))]];
  [[assume(x == 1, y)]];
}
template<class T> void t(T x) { [[assume((x, x > 0))]]; }
template void t(int);
void h(int x) {
  [[assume x]];
}
void h3(int x) {
  [[assume(x)(x)]];
}
