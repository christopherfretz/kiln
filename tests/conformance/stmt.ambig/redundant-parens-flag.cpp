//@ expect: compile-ok
//@ args: -Wredundant-parens
//@ strict-diags
//@ diag: 43:5 warning "redundant parentheses surrounding declarator"
//@ diag: 44:5 warning "redundant parentheses surrounding declarator"
//@ diag: 45:16 warning "redundant parentheses surrounding declarator"
//@ diag: 45:26 warning "redundant parentheses surrounding declarator"
//@ diag: 47:4 warning "parentheses were disambiguated as redundant parentheses around declaration of variable named 'a'"
//@ diag: 47:3 note "add enclosing parentheses to perform a function-style cast"
//@ diag: 47:4 note "remove parentheses to silence this warning"
//@ diag: 47:9 warning "redundant parentheses surrounding declarator"
//@ diag: 48:9 warning "redundant parentheses surrounding declarator"
//@ diag: 49:11 warning "redundant parentheses surrounding declarator"
//@ diag: 50:8 warning "parentheses were disambiguated as redundant parentheses around declaration of variable named 'a'"
//@ diag: 50:7 note "add enclosing parentheses to perform a function-style cast"
//@ diag: 50:8 note "remove parentheses to silence this warning"
//@ diag: 51:9 warning "parentheses were disambiguated as redundant parentheses around declaration of variable named 'b'"
//@ diag: 51:8 note "add enclosing parentheses to perform a function-style cast"
//@ diag: 51:9 note "remove parentheses to silence this warning"
//@ diag: 52:4 warning "redundant parentheses surrounding declarator"
//@ diag: 53:9 warning "parentheses were disambiguated as redundant parentheses around declaration of variable named 'e'"
//@ diag: 53:9 note "add a variable name to declare a 'W<int>' initialized with 'e'"
//@ diag: 53:3 note "add enclosing parentheses to perform a function-style cast"
//@ diag: 53:9 note "remove parentheses to silence this warning"
//@ diag: 54:4 warning "redundant parentheses surrounding declarator"
//@ diag: 55:4 warning "parentheses were disambiguated as redundant parentheses around declaration of variable named 'h'"
//@ diag: 55:3 note "add enclosing parentheses to perform a function-style cast"
//@ diag: 55:4 note "remove parentheses to silence this warning"
//@ diag: 56:7 warning "redundant parentheses surrounding declarator"
//@ diag: 57:5 warning "redundant parentheses surrounding declarator"
//@ diag: 58:5 warning "redundant parentheses surrounding declarator"
//@ diag: 60:5 warning "redundant parentheses surrounding declarator"
//@ diag: 61:10 warning "redundant parentheses surrounding declarator"
// -Wredundant-parens (off by default) and -Wvexing-parse: clang's
// warnAboutRedundantParens for every declarator whose outermost declarator
// operator is a pair of grouping parentheses. Only a block-scope,
// for-init or condition declaration of a class or dependent type with
// plain type specifiers, a first declarator naming an existing entity,
// could have been a function-style cast. WP-191.
struct T { T(); T(int); operator bool() const; };
template<class X> struct W { W(); W(int); };
int a, b, c, d, e, h, k;
int (*p1);
int (p2);
struct M { int (m1); int (*m2); };
void g() {
  T(a), (b);
  T c2, (c);
  static T(d);
  if (T(a) = 1) {}
  for (T(b);;) break;
  T(zz);
  W<int>(e);
  T(&r) = c2;
  T(*(h));
  int (q);
  T (f2(int));
  T (arr[]) = {1};
  typedef int I;
  I (x);
  const T(k);
}
