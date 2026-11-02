// [temp.variadic]/5, [expr.sub]/1: a dependent subscript with a pack
// expansion or several indices is a `CXXOperatorCallExpr` of `operator[]`
// with an empty ADL lookup set and the dependent type (clang
// `CreateOverloadedArraySubscriptExpr`); one dependent index stays an
// `ArraySubscriptExpr`. Verified against clang 19.
//@ expect: ast
//@ requires: clang>=22
template <class T, class... I> auto get(T m, I... is) { return m[is...]; }
template <class T> auto two(T m) { return m[1, 2]; }
template <class T> auto one(T m) { return m[1]; }
