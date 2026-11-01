// [dcl.ambig.res]/2, [expr.cast]: `( type-id )` whose type-id is
// ambiguous with an expression (`(T())`) is a C-style cast only when a
// cast-expression follows the `)` (clang's
// ParseCXXAmbiguousParenExpression); a `{` makes it a compound literal.
// Otherwise it is a parenthesized expression: `(T())()` calls a T,
// `(int()) / 2` divides. [dcl.ambig.res]/1: `T x(T())` is still a
// function declaration (-Wvexing-parse), `T z((T(a)))` a variable.
//@ expect: compile-fail
//@ diag: 18:6 warning "parentheses were disambiguated as a function declaration"
//@ diag: 18:7 note "add a pair of parentheses to declare a variable"
//@ diag: 19:3 error "C-style cast from 'int' to 'T ()' is not allowed"
//@ diag: 20:3 error "C-style cast from 'int' to 'T ()' is not allowed"
//@ diag: 21:3 error "C-style cast from 'int' to 'T ()' is not allowed"
//@ diag: 22:3 error "type 'T' does not provide a call operator"
struct T { T(); T(int); };
int a;
void f() {
  T x(T());
  (T())a;
  (T())*&a;
  (T())-1;
  (T())();
  int y = (int()) / 2;
  T z((T(a)));
  T w((T()));
  bool b = (int()) == 0;
  (void)y; (void)b;
}
