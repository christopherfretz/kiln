// C has no functional cast: a type keyword where an expression is
// expected is "expected expression" at the keyword, and a declarator
// without a name is "expected identifier or '('" (clang
// `ParseCastExpression`, `ParseDirectDeclarator`); a C23 call of an
// undeclared name reports it before the arguments (WP-1312, WP-1306 /
// WP-1309 follow-ups).
//@ std: c23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:9 error "use of undeclared identifier '__is_same'"
//@ diag: 17:19 error "expected expression"
//@ diag: 17:24 error "expected expression"
//@ diag: 18:9 error "use of undeclared identifier 'foo'"
//@ diag: 18:17 error "expected expression"
//@ diag: 19:6 error "expected identifier or '('"
//@ diag: 20:1 error "expected identifier or '('"
int a = __is_same(int, int);
int b = foo(1, +);
int *;
3;
