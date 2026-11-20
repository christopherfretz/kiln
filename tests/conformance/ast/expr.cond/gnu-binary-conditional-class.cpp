//@ expect: ast
// [expr.cond], [conv.rval]: GNU `x ?: y` of class type. A class prvalue
// `x` is materialized before it is shared (an xvalue `OpaqueValueExpr`);
// an lvalue `x` of the other operand's type is shared as is. WP-1738.
struct S { int i; S(int); S(const S &); ~S(); explicit operator bool() const; };
S make(int);
S &lvalue(S &a, S &z) { return a ?: z; }
S prvalue() { return make(3) ?: make(4); }
S mixed(S &a) { return a ?: make(9); }
