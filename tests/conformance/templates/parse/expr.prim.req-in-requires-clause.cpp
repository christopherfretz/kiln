//@ expect: ast
//@ oracle: clang
// [expr.prim.req.general]: a requires-expression in a requires-clause (`requires requires`)
template <class T> requires requires (T t) { t.f(); } void call(T t) { t.f(); }
