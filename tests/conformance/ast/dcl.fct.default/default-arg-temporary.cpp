//@ expect: ast
// [intro.execution]/5: the cleanups of a default argument's temporaries
// belong to the full-expression of the call, whose ExprWithCleanups
// holds them (the CXXDefaultArgExpr shows the argument without its own).
struct X { X(); ~X(); };
struct T { T(const X & = X()); ~T(); };
void g(const X & = X());
void f1() { g(); }
void f2() { T t; }
