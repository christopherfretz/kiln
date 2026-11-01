// [dcl.fct.def.general]/8: PredefinedExpr is an lvalue of type
// `const char[N]` holding its StringLiteral; in a dependent context
// (an uninstantiated template) its type is dependent and it has no
// literal.
//@ expect: ast
//@ oracle: clang
const char *f(int) { return __func__; }
struct S {
  const char *m() const { return __PRETTY_FUNCTION__; }
};
template <class T> const char *g(T) { return __FUNCTION__; }
