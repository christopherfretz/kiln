// [expr.cast]/2, [expr.comma]: the parenthesized operand of a C-style cast
// is parsed as an expression list whose comma operators are built, once
// every element is parsed, at its `(` (clang `ParseSimpleExpressionList`,
// `MaybeConvertParenListExprToParenExpr`); a comma inside a nested
// parenthesized expression keeps its own location.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:12 error "overload resolution selected deleted operator ','"
//@ diag: 21:6 note "candidate function has been explicitly deleted"
//@ diag: 25:9 error "C-style cast from 'void' to 'int' is not allowed"
//@ diag: 26:14 error "overload resolution selected deleted operator ','"
//@ diag: 21:6 note "candidate function has been explicitly deleted"
//@ diag: 27:17 error "overload resolution selected deleted operator ','"
//@ diag: 21:6 note "candidate function has been explicitly deleted"
//@ diag: 28:14 error "overload resolution selected deleted operator ','"
//@ diag: 21:6 note "candidate function has been explicitly deleted"
//@ diag: 29:9 error "overload resolution selected deleted operator ','"
//@ diag: 21:6 note "candidate function has been explicitly deleted"
//@ diag: 28:15 warning "left operand of comma operator has no effect"
struct S {};
void operator,(S, int) = delete;
struct T { void operator,(int); };
void f(S s, T t, int i) {
  (void)(S)(s, i);
  (void)(int)(t, i);
  (void)(int)(s, i, s);
  (void)(int)((s, i));
  (void)(int)(i, s, i);
  (void)(s, i);
}
