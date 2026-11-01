// [except.spec]/7 (WP-137): a call of an implicit copy or move assignment
// operator resolves its exception specification before the callee's
// DeclRefExpr type is taken, so the reference carries `noexcept`.
//@ expect: ast
struct A { int x; };
void copy(A &a, const A &b) { a = b; }
void move(A &a, A &&b) { a = static_cast<A &&>(b); }
