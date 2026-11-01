// -Wvexing-parse (clang's warnAboutRedundantParens): a block-scope
// declaration `T(x);` whose parentheses could have been a function-style
// cast of an existing `x`, with a class or dependent `T`.
//@ expect: compile-ok
//@ strict-diags
//@ xfail(kiln): the redundant-parentheses -Wvexing-parse warning is not implemented
//@ diag: 31:15 warning "parentheses were disambiguated as redundant parentheses around declaration of variable named 'a'"
//@ diag: 31:14 note "add enclosing parentheses to perform a function-style cast"
//@ diag: 31:15 note "remove parentheses to silence this warning"
//@ diag: 32:14 warning "parentheses were disambiguated as redundant parentheses around declaration of variable named 'a'"
//@ diag: 32:14 note "add a variable name to declare a 'D' initialized with 'a'"
//@ diag: 32:13 note "add enclosing parentheses to perform a function-style cast"
//@ diag: 32:14 note "remove parentheses to silence this warning"
//@ diag: 33:32 warning "parentheses were disambiguated as redundant parentheses around declaration of variable named 'a'"
//@ diag: 33:32 note "add a variable name to declare a 'U' initialized with 'a'"
//@ diag: 33:31 note "add enclosing parentheses to perform a function-style cast"
//@ diag: 33:32 note "remove parentheses to silence this warning"
//@ diag: 34:14 warning "parentheses were disambiguated as redundant parentheses around declaration of variable named 'b'"
//@ diag: 34:13 note "add enclosing parentheses to perform a function-style cast"
//@ diag: 34:14 note "remove parentheses to silence this warning"
//@ diag: 35:14 warning "parentheses were disambiguated as redundant parentheses around declaration of variable named 'a'"
//@ diag: 35:13 note "add enclosing parentheses to perform a function-style cast"
//@ diag: 35:14 note "remove parentheses to silence this warning"
//@ diag: 38:14 warning "parentheses were disambiguated as redundant parentheses around declaration of variable named 'b'"
//@ diag: 38:13 note "add enclosing parentheses to perform a function-style cast"
//@ diag: 38:14 note "remove parentheses to silence this warning"
struct T { T(); T(int); };
struct D { D(); D(int); ~D(); };
struct Q;
int a, b;
void x22() { T(a)(b); }
void x1() { D(a); }
template<class U> void d3() { U(a); }
void x2() { T(b); }
void x3() { T((a)); }
void x4() { T(a)[2]; }
void x5() { const T(a); }
void x6() { T(*b); }
void x7() { Q *q; (void)q; }
