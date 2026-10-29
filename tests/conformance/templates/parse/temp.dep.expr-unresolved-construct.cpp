//@ expect: ast
//@ oracle: clang
// [temp.dep.expr]/3, [expr.type.conv]: functional casts to a dependent type
template <class T> T a() { return T(); }
template <class T> T b(int x) { return T(x, 2); }
template <class T> T c() { return T{1, 2}; }
template <class T> T d() { return T{}; }
