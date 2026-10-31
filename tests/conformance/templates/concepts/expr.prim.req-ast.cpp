//@ expect: ast
//@ oracle: none
// [expr.prim.req]: a non-dependent requires-expression is a bool prvalue.
// oracle: none, clang-ast does not normalize non-dependent RequiresExpr yet.
struct S { int x; };
bool b = requires(S s) { s.x; };
